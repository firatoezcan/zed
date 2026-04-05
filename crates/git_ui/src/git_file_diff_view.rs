//! GitFileDiffView - A VSCode-style single file diff view.
//!
//! Shows a single file's diff in a dedicated tab using SplittableEditor.
//! The [`DiffBase`] determines what is compared:
//! - [`DiffBase::WorkingVsIndex`]: LHS = index, RHS = working copy (unstaged hunks).
//! - [`DiffBase::IndexVsHead`]: LHS = HEAD, RHS = index via a read-only scratch buffer (staged hunks).

use anyhow::{Result, anyhow};
use buffer_diff::BufferDiff;
use editor::{Editor, EditorEvent, EditorSettings, SplittableEditor};
use git::repository::RepoPath;
use gpui::{
    AnyElement, App, AppContext as _, AsyncWindowContext, Entity, EventEmitter, FocusHandle,
    Focusable, IntoElement, ParentElement as _, Render, SharedString, Styled as _, Subscription,
    Task, WeakEntity, Window,
};
use language::{Buffer, Capability, LineEnding};
use multi_buffer::MultiBuffer;
use project::ProjectPath;
use project::git_store::{GitStore, Repository};
use settings::Settings;
use std::any::{Any, TypeId};
use std::sync::Arc;
use ui::prelude::*;
use ui::{Tooltip, h_flex};
use workspace::{
    Item, ItemHandle, ItemNavHistory, ToolbarItemEvent, ToolbarItemLocation, ToolbarItemView,
    Workspace,
    item::{ItemEvent, TabContentParams},
    searchable::SearchableItemHandle,
};

/// Selects which diff the view shows.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DiffBase {
    /// Working copy vs index (unstaged hunks).
    WorkingVsIndex,
    /// Index vs HEAD (staged hunks).
    IndexVsHead,
}

impl DiffBase {
    fn label(self) -> &'static str {
        match self {
            DiffBase::WorkingVsIndex => "Working ↔ Index",
            DiffBase::IndexVsHead => "Index ↔ HEAD",
        }
    }
}

pub struct GitFileDiffView {
    repo_path: RepoPath,
    diff_base: DiffBase,
    repository: WeakEntity<Repository>,
    git_store: WeakEntity<GitStore>,
    workspace: WeakEntity<Workspace>,

    editor: Entity<SplittableEditor>,

    _focus_handle: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl GitFileDiffView {
    /// Open a single file diff view.
    ///
    /// If the active pane already contains a `GitFileDiffView` for the same
    /// `(repo_path, diff_base)` pair, that tab is activated instead of a new
    /// one being created. The newly opened (or re-activated) tab is marked as
    /// the pane's preview item.
    pub fn open(
        project_path: ProjectPath,
        repo_path: RepoPath,
        diff_base: DiffBase,
        repository: Entity<Repository>,
        workspace: WeakEntity<Workspace>,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<Result<Entity<Self>>> {
        let workspace_entity = match workspace.upgrade() {
            Some(w) => w,
            None => return Task::ready(Err(anyhow!("workspace gone"))),
        };

        // Dedup: if an existing view with the same (repo_path, diff_base)
        // is already in the active pane, re-activate it as a preview item.
        let existing = workspace_entity.update(cx, |workspace, cx| {
            let pane = workspace.active_pane().clone();
            let found = pane.read(cx).items().enumerate().find_map(|(ix, item)| {
                let view = item.downcast::<Self>()?;
                let view_ref = view.read(cx);
                if view_ref.repo_path == repo_path && view_ref.diff_base == diff_base {
                    Some((ix, view))
                } else {
                    None
                }
            });
            found.map(|(ix, view)| (pane, ix, view))
        });
        if let Some((pane, ix, view)) = existing {
            pane.update(cx, |pane, cx| {
                pane.activate_item(ix, true, true, window, cx);
            });
            return Task::ready(Ok(view));
        }

        let project = workspace_entity.read(cx).project().clone();
        let git_store = project.read(cx).git_store().clone();

        window.spawn(cx, async move |cx| {
            let (display_buffer, diff) = match diff_base {
                DiffBase::WorkingVsIndex => {
                    // Working buffer as RHS; diff base is the index text. The
                    // resulting hunks are precisely the unstaged ones.
                    let working_buffer = project
                        .update(cx, |project, cx| {
                            project.open_buffer(project_path.clone(), cx)
                        })
                        .await?;
                    let unstaged_diff = git_store
                        .update(cx, |git_store, cx| {
                            git_store.open_unstaged_diff(working_buffer.clone(), cx)
                        })
                        .await?;
                    (working_buffer, unstaged_diff)
                }
                DiffBase::IndexVsHead => {
                    // Open the project buffer only to obtain a BufferId and
                    // the file's language; the RHS is a read-only scratch
                    // buffer populated with the index text.
                    let project_buffer = project
                        .update(cx, |project, cx| {
                            project.open_buffer(project_path.clone(), cx)
                        })
                        .await?;
                    let (buffer_id, language) = project_buffer.update(cx, |buffer, _| {
                        (buffer.remote_id(), buffer.language().cloned())
                    });

                    let (head_text, index_text) = repository
                        .update(cx, |repo, cx| {
                            repo.head_and_index_text(buffer_id, repo_path.clone(), cx)
                        })
                        .await?;

                    let mut scratch_text = index_text.unwrap_or_default();
                    LineEnding::normalize(&mut scratch_text);
                    let scratch_buffer = cx.new(|cx| {
                        let mut buffer = Buffer::local(scratch_text, cx);
                        buffer.set_language(language, cx);
                        buffer.set_capability(Capability::ReadOnly, cx);
                        buffer
                    });

                    let diff =
                        Self::build_index_vs_head_diff(&scratch_buffer, head_text, cx).await?;
                    (scratch_buffer, diff)
                }
            };

            let repository_weak = repository.downgrade();
            let git_store_weak = git_store.downgrade();
            workspace_entity.update_in(cx, |workspace, window, cx| {
                let diff_view = cx.new(|cx| {
                    Self::new(
                        repo_path,
                        diff_base,
                        display_buffer,
                        diff,
                        repository_weak,
                        git_store_weak,
                        workspace,
                        window,
                        cx,
                    )
                });
                let pane = workspace.active_pane();
                pane.update(cx, |pane, cx| {
                    // Close the old preview *first* so the new item can
                    // take its slot directly. Closing after adding leaves
                    // `active_item_index` dangling one past the end.
                    let destination_index = pane.close_current_preview_item(window, cx);
                    pane.add_item(
                        Box::new(diff_view.clone()),
                        true,
                        true,
                        destination_index,
                        window,
                        cx,
                    );
                    pane.replace_preview_item_id(diff_view.item_id(), window, cx);
                });
                diff_view
            })
        })
    }

    async fn build_index_vs_head_diff(
        buffer: &Entity<Buffer>,
        head_text: Option<String>,
        cx: &mut AsyncWindowContext,
    ) -> Result<Entity<BufferDiff>> {
        let mut head_text = head_text;
        if let Some(text) = head_text.as_mut() {
            LineEnding::normalize(text);
        }
        let (language, snapshot) = buffer.update(cx, |buffer, _| {
            (buffer.language().cloned(), buffer.text_snapshot())
        });
        let diff = cx.new(|cx| BufferDiff::new(&snapshot, cx));
        let update = diff
            .update(cx, |diff, cx| {
                diff.update_diff(
                    snapshot.clone(),
                    head_text.map(|text| Arc::from(text.as_str())),
                    Some(true),
                    language,
                    cx,
                )
            })
            .await;
        diff.update(cx, |diff, cx| diff.set_snapshot(update, &snapshot, cx))
            .await;
        Ok(diff)
    }

    fn new(
        repo_path: RepoPath,
        diff_base: DiffBase,
        new_buffer: Entity<Buffer>,
        diff: Entity<BufferDiff>,
        repository: WeakEntity<Repository>,
        git_store: WeakEntity<GitStore>,
        workspace: &Workspace,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        let workspace_weak = workspace.weak_handle();
        let workspace_handle = workspace
            .weak_handle()
            .upgrade()
            .expect("workspace should be valid");
        let project = workspace.project().clone();

        let multibuffer = cx.new(|cx| {
            let mut multibuffer = MultiBuffer::singleton(new_buffer.clone(), cx);
            multibuffer.add_diff(diff.clone(), cx);
            multibuffer.set_all_diff_hunks_expanded(cx);
            multibuffer
        });

        let editor = cx.new(|cx| {
            let splittable_editor = SplittableEditor::new(
                EditorSettings::get_global(cx).diff_view_style,
                multibuffer.clone(),
                project,
                workspace_handle,
                window,
                cx,
            );
            splittable_editor.rhs_editor().update(cx, |editor, cx| {
                editor.disable_diagnostics(cx);
                editor.start_temporary_diff_override();
                editor.set_expand_all_diff_hunks(cx);
            });
            splittable_editor
        });

        let editor_subscription = cx.subscribe(&editor, |_, _, event: &EditorEvent, cx| {
            cx.emit(event.clone());
        });

        let _ = new_buffer;
        let _ = diff;

        Self {
            repo_path,
            diff_base,
            repository,
            git_store,
            workspace: workspace_weak,
            editor,
            _focus_handle: focus_handle,
            _subscriptions: vec![editor_subscription],
        }
    }

    /// Navigate to the most recent commit that touched this file, opening a
    /// `CommitView` filtered to just this file's changes.
    pub fn view_last_commit(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) {
        let repository = self.repository.clone();
        let workspace = self.workspace.clone();
        let git_store = self.git_store.clone();
        let repo_path = self.repo_path.clone();

        let load_task: Option<Task<Result<git::repository::FileHistory>>> =
            match git_store.upgrade() {
                Some(gs) => gs.update(cx, |git_store, cx| {
                    let repo = repository.upgrade()?;
                    let task = git_store
                        .file_history_paginated(&repo, repo_path.clone(), 0, Some(1), cx);
                    Some(task)
                }),
                None => None,
            };

        let Some(load_task) = load_task else {
            return;
        };

        cx.spawn_in(window, async move |_, cx| {
            let history = load_task.await?;
            let Some(first_entry) = history.entries.first() else {
                return Ok(());
            };
            let commit_sha = first_entry.sha.to_string();

            cx.update(|window, cx| {
                crate::commit_view::CommitView::open(
                    commit_sha,
                    repository,
                    workspace,
                    None,
                    Some(repo_path),
                    window,
                    cx,
                );
            })?;
            anyhow::Ok(())
        })
        .detach();
    }

    fn display_title(&self) -> SharedString {
        let file_name = self.repo_path.file_name().unwrap_or("untitled");
        format!("{} ({})", file_name, self.diff_base.label()).into()
    }
}

impl EventEmitter<EditorEvent> for GitFileDiffView {}

impl Focusable for GitFileDiffView {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.editor.read(cx).focused_editor().focus_handle(cx)
    }
}

impl Item for GitFileDiffView {
    type Event = EditorEvent;

    fn tab_icon(&self, _: &Window, _: &App) -> Option<Icon> {
        Some(Icon::new(IconName::Diff).color(Color::Muted))
    }

    fn tab_content(&self, params: TabContentParams, _: &Window, _: &App) -> AnyElement {
        Label::new(self.display_title())
            .color(if params.selected {
                Color::Default
            } else {
                Color::Muted
            })
            .into_any_element()
    }

    fn tab_content_text(&self, _: usize, _: &App) -> SharedString {
        self.display_title()
    }

    fn tab_tooltip_text(&self, _: &App) -> Option<SharedString> {
        Some(self.display_title())
    }

    fn to_item_events(event: &EditorEvent, f: &mut dyn FnMut(ItemEvent)) {
        Editor::to_item_events(event, f)
    }

    fn telemetry_event_text(&self) -> Option<&'static str> {
        Some("Git File Diff View Opened")
    }

    fn preserve_preview(&self, _cx: &App) -> bool {
        // The diff view itself is not user-editable — `BufferRangesUpdated`
        // events get emitted as the multibuffer finishes loading diff hunks,
        // which would otherwise un-preview the tab immediately.
        true
    }

    fn deactivated(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) {
        self.editor.update(cx, |editor, cx| {
            editor
                .rhs_editor()
                .update(cx, |editor, cx| editor.deactivated(window, cx))
        });
    }

    fn act_as_type<'a>(
        &'a self,
        type_id: TypeId,
        self_handle: &'a Entity<Self>,
        cx: &'a App,
    ) -> Option<gpui::AnyEntity> {
        if type_id == TypeId::of::<Self>() {
            Some(self_handle.clone().into())
        } else if type_id == TypeId::of::<Editor>() {
            Some(self.editor.read(cx).rhs_editor().clone().into())
        } else {
            None
        }
    }

    fn as_searchable(&self, _: &Entity<Self>, cx: &App) -> Option<Box<dyn SearchableItemHandle>> {
        Some(Box::new(self.editor.read(cx).rhs_editor().clone()))
    }

    fn for_each_project_item(
        &self,
        cx: &App,
        f: &mut dyn FnMut(gpui::EntityId, &dyn project::ProjectItem),
    ) {
        self.editor
            .read(cx)
            .rhs_editor()
            .for_each_project_item(cx, f)
    }

    fn set_nav_history(
        &mut self,
        nav_history: ItemNavHistory,
        _: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) {
        self.editor.update(cx, |editor, cx| {
            editor.rhs_editor().update(cx, |editor, _| {
                editor.set_nav_history(Some(nav_history));
            });
        });
    }

    fn navigate(
        &mut self,
        data: Arc<dyn Any + Send>,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> bool {
        self.editor.update(cx, |editor, cx| {
            editor
                .rhs_editor()
                .update(cx, |editor, cx| editor.navigate(data, window, cx))
        })
    }
}

impl Render for GitFileDiffView {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        v_flex().size_full().child(self.editor.clone())
    }
}

/// Toolbar that provides quick access to "last commit that touched this
/// file". Activates for `GitFileDiffView` tabs *and* for any regular
/// `Editor` tab whose buffer corresponds to a file in a git repository.
pub struct GitFileDiffViewToolbar {
    target: ToolbarTarget,
    workspace: WeakEntity<Workspace>,
}

enum ToolbarTarget {
    None,
    DiffView(WeakEntity<GitFileDiffView>),
    File(ProjectPath),
}

impl GitFileDiffViewToolbar {
    pub fn new(workspace: &Workspace, _: &mut gpui::Context<Self>) -> Self {
        Self {
            target: ToolbarTarget::None,
            workspace: workspace.weak_handle(),
        }
    }

    fn open_last_commit_for_file(
        &self,
        project_path: ProjectPath,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(workspace) = self.workspace.upgrade() else {
            return;
        };
        let project = workspace.read(cx).project().clone();
        let git_store = project.read(cx).git_store().clone();
        let Some((repo, repo_path)) = git_store
            .read(cx)
            .repository_and_path_for_project_path(&project_path, cx)
        else {
            return;
        };
        let load_task = git_store.update(cx, |git_store, cx| {
            git_store.file_history_paginated(&repo, repo_path.clone(), 0, Some(1), cx)
        });
        let repository_weak = repo.downgrade();
        let workspace_weak = self.workspace.clone();
        window
            .spawn(cx, async move |cx| {
                let history = load_task.await?;
                let Some(first_entry) = history.entries.first() else {
                    return anyhow::Ok(());
                };
                let commit_sha = first_entry.sha.to_string();
                cx.update(|window, cx| {
                    crate::commit_view::CommitView::open(
                        commit_sha,
                        repository_weak,
                        workspace_weak,
                        None,
                        Some(repo_path),
                        window,
                        cx,
                    );
                })?;
                anyhow::Ok(())
            })
            .detach();
    }
}

impl EventEmitter<ToolbarItemEvent> for GitFileDiffViewToolbar {}

impl Render for GitFileDiffViewToolbar {
    fn render(&mut self, _window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        if matches!(self.target, ToolbarTarget::None) {
            return h_flex();
        }
        h_flex().gap_1().child(
            IconButton::new("view-last-commit", IconName::HistoryRerun)
                .icon_size(IconSize::Small)
                .tooltip(Tooltip::text("View the last commit that touched this file"))
                .on_click(cx.listener(|this, _, window, cx| match &this.target {
                    ToolbarTarget::DiffView(view) => {
                        if let Some(view) = view.upgrade() {
                            view.update(cx, |view, cx| view.view_last_commit(window, cx));
                        }
                    }
                    ToolbarTarget::File(project_path) => {
                        this.open_last_commit_for_file(project_path.clone(), window, cx);
                    }
                    ToolbarTarget::None => {}
                })),
        )
    }
}

impl ToolbarItemView for GitFileDiffViewToolbar {
    fn set_active_pane_item(
        &mut self,
        active_pane_item: Option<&dyn ItemHandle>,
        _: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> ToolbarItemLocation {
        self.target = ToolbarTarget::None;
        let Some(item) = active_pane_item else {
            return ToolbarItemLocation::Hidden;
        };
        if let Some(entity) = item.act_as::<GitFileDiffView>(cx) {
            self.target = ToolbarTarget::DiffView(entity.downgrade());
            return ToolbarItemLocation::PrimaryRight;
        }
        // For regular Editor tabs (and anything else backed by a file),
        // only show the button when the file actually lives in a git
        // repository inside the project.
        if let Some(project_path) = item.project_path(cx)
            && let Some(workspace) = self.workspace.upgrade()
        {
            let project = workspace.read(cx).project().clone();
            let git_store = project.read(cx).git_store();
            if git_store
                .read(cx)
                .repository_and_path_for_project_path(&project_path, cx)
                .is_some()
            {
                self.target = ToolbarTarget::File(project_path);
                return ToolbarItemLocation::PrimaryRight;
            }
        }
        ToolbarItemLocation::Hidden
    }

    fn pane_focus_update(
        &mut self,
        _pane_focused: bool,
        _window: &mut Window,
        _cx: &mut gpui::Context<Self>,
    ) {
    }
}

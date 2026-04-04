//! GitFileDiffView - A VSCode-style single file diff view.
//!
//! Shows a single file's diff in a dedicated tab using SplittableEditor.
//! Left side: HEAD content (read-only). Right side: working copy.
//! Has a "Previous Commit" button to navigate through file history.

use anyhow::Result;
use buffer_diff::BufferDiff;
use editor::{Editor, EditorEvent, EditorSettings, SplittableEditor};
use git::repository::RepoPath;
use gpui::{
    AnyElement, App, AppContext as _, Entity, EventEmitter, FocusHandle, Focusable, IntoElement,
    ParentElement as _, Render, SharedString, Styled as _, Subscription, Task, WeakEntity, Window,
    div, px,
};
use language::Buffer;
use multi_buffer::MultiBuffer;
use project::ProjectPath;
use project::git_store::{GitStore, Repository};
use settings::Settings;
use std::any::{Any, TypeId};
use std::sync::Arc;
use ui::{
    ActiveTheme as _, Button, ButtonCommon as _, Clickable as _, Color, Icon, IconName, Label,
    LabelCommon as _, LabelSize, Tooltip, v_flex,
};
use workspace::{
    Item, ItemHandle as _, ItemNavHistory, Workspace,
    item::{ItemEvent, TabContentParams},
    searchable::SearchableItemHandle,
};

pub struct GitFileDiffView {
    repo_path: RepoPath,
    repository: WeakEntity<Repository>,
    git_store: WeakEntity<GitStore>,
    workspace: WeakEntity<Workspace>,

    editor: Entity<SplittableEditor>,

    _focus_handle: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl GitFileDiffView {
    /// Open a single file diff view for working copy vs HEAD.
    pub fn open(
        project_path: ProjectPath,
        repo_path: RepoPath,
        repository: Entity<Repository>,
        workspace: WeakEntity<Workspace>,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<Result<Entity<Self>>> {
        let workspace_entity = match workspace.upgrade() {
            Some(w) => w,
            None => return Task::ready(Err(anyhow::anyhow!("workspace gone"))),
        };
        let project = workspace_entity.read(cx).project().clone();
        let git_store = project.read(cx).git_store().clone();

        window.spawn(cx, async move |cx| {
            // Open the working copy buffer
            let new_buffer = project
                .update(cx, |project, cx| {
                    project.open_buffer(project_path.clone(), cx)
                })
                .await?;

            // Create a diff with HEAD as the base (None means HEAD)
            let diff = git_store
                .update(cx, |git_store, cx| {
                    git_store.open_diff_since(None, new_buffer.clone(), repository.clone(), cx)
                })
                .await?;

            let repository_weak = repository.downgrade();
            let git_store_weak = git_store.downgrade();
            workspace_entity.update_in(cx, |workspace, window, cx| {
                let diff_view = cx.new(|cx| {
                    Self::new(
                        repo_path,
                        new_buffer,
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
                    pane.add_item(Box::new(diff_view.clone()), true, true, None, window, cx);
                });
                diff_view
            })
        })
    }

    fn new(
        repo_path: RepoPath,
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
            repository,
            git_store,
            workspace: workspace_weak,
            editor,
            _focus_handle: focus_handle,
            _subscriptions: vec![editor_subscription],
        }
    }

    /// Open the file history view for this file.
    fn view_file_history(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) {
        crate::file_history_view::FileHistoryView::open(
            self.repo_path.clone(),
            self.git_store.clone(),
            self.repository.clone(),
            self.workspace.clone(),
            window,
            cx,
        );
    }

    /// Navigate to the previous commit of this file by opening a CommitView
    /// for the most recent commit of this file, filtered to show only this file's changes.
    fn view_last_commit(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) {
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
        format!("{} (Working ↔ HEAD)", file_name).into()
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
    fn render(&mut self, _: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_1()
                    .h(px(36.))
                    .flex_none()
                    .border_b_1()
                    .border_color(cx.theme().colors().border)
                    .bg(cx.theme().colors().editor_background)
                    .child(
                        Button::new("last-commit-btn", "← Last Commit")
                            .tooltip(|_, cx| {
                                Tooltip::simple(
                                    "View the last commit's changes for this file",
                                    cx,
                                )
                            })
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.view_last_commit(window, cx);
                            })),
                    )
                    .child(
                        Button::new("file-history-btn", "All History")
                            .tooltip(|_, cx| {
                                Tooltip::simple("View the full commit history of this file", cx)
                            })
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.view_file_history(window, cx);
                            })),
                    )
                    .child(
                        Label::new(self.display_title())
                            .size(LabelSize::Small)
                            .color(Color::Muted),
                    ),
            )
            .child(self.editor.clone())
    }
}

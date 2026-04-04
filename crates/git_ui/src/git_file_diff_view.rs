//! GitFileDiffView - A VSCode-style single file diff view.
//!
//! Shows a single file's diff in a dedicated tab using SplittableEditor.
//! Left side: HEAD content (read-only). Right side: working copy.

use anyhow::Result;
use buffer_diff::BufferDiff;
use editor::{Editor, EditorEvent, EditorSettings, SplittableEditor};
use git::repository::RepoPath;
use gpui::{
    AnyElement, App, AppContext as _, Entity, EventEmitter, FocusHandle, Focusable, IntoElement,
    Render, SharedString, Subscription, Task, WeakEntity, Window,
};
use language::Buffer;
use multi_buffer::MultiBuffer;
use project::ProjectPath;
use project::git_store::Repository;
use settings::Settings;
use std::any::{Any, TypeId};
use std::sync::Arc;
use ui::{Color, Icon, IconName, Label, LabelCommon as _};
use workspace::{
    Item, ItemHandle as _, ItemNavHistory, Workspace,
    item::{ItemEvent, TabContentParams},
    searchable::SearchableItemHandle,
};

pub struct GitFileDiffView {
    repo_path: RepoPath,
    editor: Entity<SplittableEditor>,
    _new_buffer: Entity<Buffer>,
    _diff: Entity<BufferDiff>,
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

            workspace_entity.update_in(cx, |workspace, window, cx| {
                let diff_view = cx.new(|cx| {
                    Self::new(
                        repo_path,
                        new_buffer,
                        diff,
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
        workspace: &Workspace,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        let project = workspace.project().clone();
        let workspace_entity = cx.entity().downgrade();
        let workspace_handle = workspace
            .weak_handle()
            .upgrade()
            .expect("workspace should be valid at this point");

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
                project.clone(),
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

        let _ = workspace_entity;

        Self {
            repo_path,
            editor,
            _new_buffer: new_buffer,
            _diff: diff,
            _focus_handle: focus_handle,
            _subscriptions: vec![editor_subscription],
        }
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
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        self.editor.clone()
    }
}

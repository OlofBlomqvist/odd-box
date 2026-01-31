use iced::widget::{column, container, text};
use iced::{Element, Length, Task, Theme};
use std::sync::Arc;

use crate::global_state::GlobalState;

pub fn run(state: Arc<GlobalState>) -> iced::Result {
    iced::application("odd-box", OddBoxGui::update, OddBoxGui::view)
        .theme(OddBoxGui::theme)
        .run_with(move || OddBoxGui::new(state))
}

#[derive(Debug, Clone)]
pub enum Message {
    // We'll add more messages as we build out the UI
}

pub struct OddBoxGui {
    #[allow(dead_code)]
    state: Arc<GlobalState>,
}

impl OddBoxGui {
    fn new(state: Arc<GlobalState>) -> (Self, Task<Message>) {
        (Self { state }, Task::none())
    }

    fn update(&mut self, _message: Message) -> Task<Message> {
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let title = text("odd-box")
            .size(32);

        let subtitle = text("reverse proxy & process manager")
            .size(16);

        let status = text("GUI is working!")
            .size(14);

        let content = column![title, subtitle, status]
            .spacing(10)
            .padding(20);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    }

    fn theme(&self) -> Theme {
        Theme::default()
    }
}

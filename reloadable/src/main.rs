
pub mod hot_ice;
pub mod reloader;
pub mod unsafe_reference;
pub mod reloadable;
pub mod lib_reloader;
pub mod lib_reload_events;
pub mod hot_view;
pub mod error;

use crossfire::mpmc::{RxBlocking, RxFuture, SharedSenderBRecvF, SharedSenderFRecvB, TxBlocking, TxFuture};
use hot_lib_reloader::LibReloadObserver;
use iced::{application::Boot, futures::{SinkExt, Stream}, stream, widget::{button, column, container, text}, Length, Task};
use once_cell::sync::OnceCell;
use app::*;


#[hot_lib_reloader::hot_module(dylib = "ui", lib_dir = "target/debug")]
mod app {
    pub use ui::*; 
    // hot_functions_from_file!("ui/src/lib.rs", ignore_no_mangle = true);

    #[hot_functions]
    extern "Rust" {
        pub fn view(state: &Names) -> Element<Message>;
        pub fn update(state: &mut Names, message: ui::Message) -> Task<ui::Message>;
    }



    #[lib_change_subscription]
    pub fn subscribe() -> hot_lib_reloader::LibReloadObserver {}
}


// #[derive(Debug, Clone)]
// pub enum Message {
//     None,
//     Reloading,
//     ReloadFinished,
//     SendReadySignal,
//     AppMessage(app::Message),
// }

// struct ReloadableInner {
//     app: Names,
//     is_reloading: bool,
//     update_ch_tx: TxFuture<ReadyToReload, SharedSenderFRecvB>,
// }

// impl ReloadableInner {
//     pub fn new() -> Self {
//         let (update_ch_tx, _) = UPDATE_CHANNEL.get().unwrap().clone();
//         Self {
//             app: Names::new(),
//             is_reloading: false,
//             update_ch_tx,
//         }
//     }

//     pub fn update(&mut self, message: Message) -> Task<Message> {
//         match message {
//             Message::AppMessage(message) => {
//                 if self.is_reloading {return Task::none()}
//                 app::update(&mut self.app, message).map(Message::AppMessage)
//             }
//             Message::Reloading => {
//                 self.is_reloading = true;
//                 Task::done(Message::SendReadySignal)
//             }
//             Message::SendReadySignal => {
//                 let sender = self.update_ch_tx.clone();
//                 Task::future(async move {sender.send(ReadyToReload).await}).discard()
//             }
//             Message::ReloadFinished => {
//                 self.is_reloading = false;
//                 Task::none()
//             }
//             Message::None => {
//                 Task::none()
//             }
//         }
//     }

//     pub fn view(&self) -> Element<Message> {
//         if self.is_reloading {
//             container(column![
//                 text("Reloading...").size(20),
//                 button("Refresh").on_press(Message::None)
//             ])
//             .center_x(Length::Fill)
//             .center_y(Length::Fill)
//             .into()
//         } else {
//             app::view(&self.app).map(Message::AppMessage)
//         }
//     }

//     pub fn subscription(&self) -> iced::Subscription<Message> {
//         iced::Subscription::run( listen_for_lib_change)
//     }

// }

// fn listen_for_lib_change() -> impl Stream<Item = Message> {
//     let rx = SUBSCRIPTION_CHANNEL.get().unwrap().1.clone();
//     stream::channel(10, async move |mut output| {
//         loop {
//             match rx.recv().await {
//                 Ok(message) => {
//                     match message {
//                         ReloadEvent::AboutToReload => {
//                             if let Err(err) = output.send(Message::Reloading).await {
//                                 println!("Failed to send reloading message: {err}")
//                             }
//                         }
//                         ReloadEvent::ReloadComplete => {
//                             if let Err(err) = output.send(Message::ReloadFinished).await {
//                                 println!("Failed to send reload complete message: {err}")
//                             }
//                         }
//                         _ => {}
//                     }
//                 }
//                 Err(err) => {
//                     println!("{err}")
//                 }
//             }
//         }
//     })
// }

// static SUBSCRIPTION_CHANNEL: OnceCell<(TxBlocking<ReloadEvent,SharedSenderBRecvF>, RxFuture<ReloadEvent, SharedSenderBRecvF>)> = OnceCell::new();
// static UPDATE_CHANNEL: OnceCell<(TxFuture<ReadyToReload, SharedSenderFRecvB>, RxBlocking<ReadyToReload, SharedSenderFRecvB>)> = OnceCell::new();

// #[derive(Debug, Clone, PartialEq)]
// enum ReloadEvent {
//     AboutToReload,
//     ReloadComplete
// }

// struct ReadyToReload;

// struct HotIce;

// impl HotIce {
//     pub fn new(lib_observer: LibReloadObserver) -> Self {
//         let (subscription_ch_tx, _)  = SUBSCRIPTION_CHANNEL.get_or_init(||crossfire::mpmc::bounded_tx_blocking_rx_future(1)).clone();
//         let (_, update_ch_rx) = UPDATE_CHANNEL.get_or_init(|| crossfire::mpmc::bounded_tx_future_rx_blocking(1)).clone();

//         std::thread::spawn(move || {
//             loop {
//                 println!("Waiting for reload");
//                 let blocker = lib_observer.wait_for_about_to_reload();
//                 if let Err(err) = subscription_ch_tx.send(ReloadEvent::AboutToReload) {
//                     println!("{err}")
//                 }
                
//                 println!("Waiting for reload signal");
//                 let Ok(ReadyToReload) = update_ch_rx.recv() else {
//                     panic!("Wrong reload event received")
//                 };

//                 drop(blocker);
//                 println!("Reloading lib");

//                 lib_observer.wait_for_reload();
//                 println!("Reload complete");
//                 if let Err(err) = subscription_ch_tx.send(ReloadEvent::ReloadComplete) {
//                     println!("{err}")
//                 }
//             }
//         });
        
//         Self
//     }

//     pub fn run(self) -> Result<(), iced::Error> {
//         iced::application(
//             ReloadableInner::new, 
//             ReloadableInner::update, 
//             ReloadableInner::view
//         )
//         .subscription(ReloadableInner::subscription)
//         .run()
//     }
// }


// fn main() {
//     let lib_observer = app::subscribe();

//     HotIce::new(lib_observer).run().unwrap();
// }

fn main() {
    hot_ice::application("ui", "ui", Names::new, Names::update, Names::view).run().unwrap();
}
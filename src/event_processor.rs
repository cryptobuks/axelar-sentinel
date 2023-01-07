use crate::event_sub::Event;
use async_trait::async_trait;
use core::future::Future;
use core::pin::Pin;
use error_stack::{IntoReport, Report, Result, ResultExt};
use futures::{future::try_join_all, StreamExt};
use std::vec;
use thiserror::Error;
use tokio::{select, sync::oneshot};
use tokio_stream::wrappers::BroadcastStream;

#[derive(Error, Debug)]
pub enum EventProcessorError {
    #[error("event handler {name} failed handling event")]
    EventHandlerError { name: String },
    #[error("event handler task error")]
    EventHandlerJoinError,
    #[error("event stream error")]
    EventStreamError,
    #[error("failed closing event processor")]
    CloseFailed,
}

#[async_trait]

pub trait EventHandler {
    type Err;

    fn name(&self) -> &str;
    fn should_handle(&self, event: &Event) -> bool;
    async fn handle(&self, event: &Event) -> Result<(), Self::Err>;
}

pub struct EventProcessorDriver {
    close_tx: oneshot::Sender<()>,
}

impl EventProcessorDriver {
    pub fn close(self) -> Result<(), EventProcessorError> {
        self.close_tx
            .send(())
            .map_err(|_| Report::new(EventProcessorError::CloseFailed))
    }
}

type Task = Box<dyn Future<Output = Result<(), EventProcessorError>> + Send>;

fn new_task<E: 'static>(
    handler: Box<dyn EventHandler<Err = E> + Send + Sync>,
    mut event_stream: BroadcastStream<Event>,
) -> Task {
    let task = async move {
        loop {
            match event_stream.next().await {
                None => {
                    return Ok(());
                }
                Some(Err(err)) => {
                    return Err(err)
                        .into_report()
                        .change_context(EventProcessorError::EventStreamError);
                }
                Some(Ok(event)) => {
                    if !handler.should_handle(&event) {
                        continue;
                    }

                    handler
                        .handle(&event)
                        .await
                        .change_context(EventProcessorError::EventHandlerError {
                            name: handler.name().to_string(),
                        })?;
                }
            }
        }
    };

    Box::new(task)
}

pub struct EventProcessor {
    tasks: Vec<Pin<Task>>,
    close_rx: oneshot::Receiver<()>,
}

impl EventProcessor {
    pub fn new() -> (Self, EventProcessorDriver) {
        let (close_tx, close_rx) = oneshot::channel();

        (
            EventProcessor {
                tasks: vec![],
                close_rx,
            },
            EventProcessorDriver { close_tx },
        )
    }

    pub fn add_handler<E: 'static>(
        mut self,
        handler: Box<dyn EventHandler<Err = E> + Send + Sync>,
        event_stream: BroadcastStream<Event>,
    ) -> Self {
        self.tasks.push(new_task(handler, event_stream).into());
        self
    }

    pub async fn run(mut self) -> Result<(), EventProcessorError> {
        let handles = self.tasks.into_iter().map(tokio::spawn);

        select! {
            result = try_join_all(handles) => {
                match result {
                    Err(err) => {
                        Err(err).into_report().change_context(EventProcessorError::EventHandlerJoinError)
                    },
                    Ok(results) => {
                        results.into_iter().find(|result| result.is_err()).unwrap_or(Ok(()))
                    },
                }
            },
            _ = &mut self.close_rx => {
                Ok(())
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::event_processor;
    use crate::event_sub;
    use async_trait::async_trait;
    use error_stack::{IntoReport, Result};
    use mockall::mock;
    use thiserror::Error;
    use tokio::{self, sync::broadcast};
    use tokio_stream::wrappers::BroadcastStream;

    #[tokio::test]
    async fn should_not_handle_events_filtered_out() {
        let event_count = 10;
        let (tx, rx) = broadcast::channel::<event_sub::Event>(event_count);
        let (processor, driver) = event_processor::EventProcessor::new();

        let mut handler = MockEventHandler::new();
        handler.expect_should_handle().return_const(false).times(event_count);

        tokio::spawn(async move {
            for i in 0..event_count {
                assert!(tx.send(event_sub::Event::BlockEnd((i as u32).into())).is_ok());
            }
            assert!(driver.close().is_ok());
        });

        assert!(processor
            .add_handler(Box::new(handler), BroadcastStream::new(rx))
            .run()
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn should_handle_events_not_filtered_out() {
        let event_count = 10;
        let (tx, rx) = broadcast::channel::<event_sub::Event>(event_count);
        let (processor, driver) = event_processor::EventProcessor::new();

        let mut handler = MockEventHandler::new();
        handler.expect_should_handle().return_const(true).times(event_count);
        handler.expect_handle().returning(|_| Ok(())).times(event_count);
        handler.expect_name().return_const("handler".into());

        tokio::spawn(async move {
            for i in 0..event_count {
                assert!(tx.send(event_sub::Event::BlockEnd((i as u32).into())).is_ok());
            }
            assert!(driver.close().is_ok());
        });

        assert!(processor
            .add_handler(Box::new(handler), BroadcastStream::new(rx))
            .run()
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn should_return_error_if_handler_fails() {
        let (tx, rx) = broadcast::channel::<event_sub::Event>(10);
        let (processor, _driver) = event_processor::EventProcessor::new();

        let mut handler = MockEventHandler::new();
        handler.expect_should_handle().return_const(true).once();
        handler
            .expect_handle()
            .returning(|_| Err(EventHandlerError::Unknown).into_report())
            .once();
        handler.expect_name().return_const("handler".into());

        tokio::spawn(async move {
            assert!(tx.send(event_sub::Event::BlockEnd((10_u32).into())).is_ok());
        });

        assert!(processor
            .add_handler(Box::new(handler), BroadcastStream::new(rx))
            .run()
            .await
            .is_err());
    }

    #[tokio::test]
    async fn should_support_multiple_types_of_handlers() {
        let event_count = 10;
        let (tx, rx) = broadcast::channel::<event_sub::Event>(event_count);
        let (processor, driver) = event_processor::EventProcessor::new();
        let stream = BroadcastStream::new(rx);
        let another_stream = BroadcastStream::new(tx.subscribe());

        let mut handler = MockEventHandler::new();
        handler.expect_should_handle().return_const(false).times(event_count);

        let mut another_handler = MockAnotherEventHandler::new();
        another_handler
            .expect_should_handle()
            .return_const(true)
            .times(event_count);
        another_handler.expect_handle().returning(|_| Ok(())).times(event_count);
        another_handler.expect_name().return_const("another handler".into());

        tokio::spawn(async move {
            for i in 0..event_count {
                assert!(tx.send(event_sub::Event::BlockEnd((i as u32).into())).is_ok());
            }
            assert!(driver.close().is_ok());
        });

        assert!(processor
            .add_handler(Box::new(handler), stream)
            .add_handler(Box::new(another_handler), another_stream)
            .run()
            .await
            .is_ok());
    }

    #[derive(Error, Debug)]
    pub enum EventHandlerError {
        #[error("unknown")]
        Unknown,
    }

    mock! {
            EventHandler{}

            #[async_trait]
            impl event_processor::EventHandler for EventHandler {
                type Err = EventHandlerError;

                fn name(&self) -> &str;
                fn should_handle(&self, event: &event_sub::Event) -> bool;
                async fn handle(&self, event: &event_sub::Event) -> Result<(), EventHandlerError>;
            }
    }

    #[derive(Error, Debug)]
    pub enum AnotherEventHandlerError {}

    mock! {
            AnotherEventHandler{}

            #[async_trait]
            impl event_processor::EventHandler for AnotherEventHandler {
                type Err = AnotherEventHandlerError;

                fn name(&self) -> &str;
                fn should_handle(&self, event: &event_sub::Event) -> bool;
                async fn handle(&self, event: &event_sub::Event) -> Result<(), AnotherEventHandlerError>;
            }
    }
}

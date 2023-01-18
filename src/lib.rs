use error_stack::Result;

use crate::report::Error;

pub mod config;
pub mod event_sub;
pub mod report;
pub mod tm_client;
pub mod tofnd {
    include!(concat!(env!("OUT_DIR"), "/tofnd.rs"));
}
pub mod axelar {
    pub mod utils {
        pub mod v1beta1 {
            include!(concat!(env!("OUT_DIR"), "/axelar.utils.v1beta1.rs"));
        }
    }
    pub mod snapshot {
        pub mod exported {
            pub mod v1beta1 {
                include!(concat!(env!("OUT_DIR"), "/axelar.snapshot.exported.v1beta1.rs"));
            }
        }
    }
    pub mod vote {
        pub mod exported {
            pub mod v1beta1 {
                include!(concat!(env!("OUT_DIR"), "/axelar.vote.exported.v1beta1.rs"));
            }
        }
        pub mod v1beta1 {
            include!(concat!(env!("OUT_DIR"), "/axelar.vote.v1beta1.rs"));
        }
    }
}

pub fn run(_cfg: config::Config) -> Result<(), Error> {
    unimplemented!()
}

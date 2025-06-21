use std::error::Error;

pub type ServiceError = Box<dyn Error + Send + Sync>;

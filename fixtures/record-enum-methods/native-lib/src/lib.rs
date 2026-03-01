#[derive(uniffi::Record, Clone, Debug)]
pub struct MethodPoint {
    pub x: u32,
    pub y: u32,
}

#[derive(uniffi::Enum, Clone, Debug)]
pub enum MethodState {
    Idle,
    Busy,
}

#[derive(uniffi::Enum, Clone, Debug)]
pub enum MethodOutcome {
    Ok { value: u32 },
    Err { message: String },
}

#[derive(Debug, thiserror::Error, uniffi::Error, Clone)]
pub enum MethodError {
    #[error("DivisionByZero")]
    DivisionByZero,
    #[error("NegativeInput")]
    NegativeInput { value: i32 },
}

pub struct MethodLabel(pub String);
uniffi::custom_newtype!(MethodLabel, String);

type MethodResult<T, E = MethodError> = std::result::Result<T, E>;

#[uniffi::export]
impl MethodPoint {
    fn checksum(&self) -> u32 {
        self.x.saturating_mul(31).saturating_add(self.y)
    }

    fn checked_divide(&self, divisor: u32) -> MethodResult<u32> {
        if divisor == 0 {
            return Err(MethodError::DivisionByZero);
        }
        let sum = self.x.saturating_add(self.y);
        Ok(sum / divisor)
    }

    #[uniffi::method(async_runtime = "tokio")]
    async fn async_label(&self, prefix: MethodLabel) -> MethodLabel {
        MethodLabel(format!("{}:{}:{}", prefix.0, self.x, self.y))
    }

    fn as_state(&self) -> MethodState {
        if self.x.saturating_add(self.y) == 0 {
            MethodState::Idle
        } else {
            MethodState::Busy
        }
    }
}

#[uniffi::export]
impl MethodState {
    fn weight(&self) -> u32 {
        match self {
            Self::Idle => 1,
            Self::Busy => 9,
        }
    }

    fn checked_code(&self, allow_busy: bool) -> MethodResult<u32> {
        match (self, allow_busy) {
            (Self::Idle, _) => Ok(100),
            (Self::Busy, true) => Ok(900),
            (Self::Busy, false) => Err(MethodError::NegativeInput { value: -9 }),
        }
    }

    #[uniffi::method(async_runtime = "tokio")]
    async fn async_label(&self, prefix: MethodLabel) -> MethodLabel {
        let suffix = match self {
            Self::Idle => "idle",
            Self::Busy => "busy",
        };
        MethodLabel(format!("{}:{}", prefix.0, suffix))
    }

    fn to_outcome(&self) -> MethodOutcome {
        match self {
            Self::Idle => MethodOutcome::Ok { value: 1 },
            Self::Busy => MethodOutcome::Err {
                message: "busy".to_string(),
            },
        }
    }
}

#[uniffi::export]
impl MethodOutcome {
    fn score(&self) -> u32 {
        match self {
            Self::Ok { value } => *value,
            Self::Err { message } => message.len() as u32,
        }
    }

    fn checked_value(&self) -> MethodResult<u32> {
        match self {
            Self::Ok { value } => Ok(*value),
            Self::Err { .. } => Err(MethodError::NegativeInput { value: -1 }),
        }
    }

    #[uniffi::method(async_runtime = "tokio")]
    async fn async_label(&self, prefix: MethodLabel) -> MethodLabel {
        match self {
            Self::Ok { value } => MethodLabel(format!("{}:ok:{}", prefix.0, value)),
            Self::Err { message } => MethodLabel(format!("{}:err:{}", prefix.0, message)),
        }
    }
}

#[uniffi::export]
fn method_point_new(x: u32, y: u32) -> MethodPoint {
    MethodPoint { x, y }
}

#[uniffi::export]
fn method_state_busy() -> MethodState {
    MethodState::Busy
}

#[uniffi::export]
fn method_outcome_ok(value: u32) -> MethodOutcome {
    MethodOutcome::Ok { value }
}

uniffi::setup_scaffolding!("record_enum_methods");

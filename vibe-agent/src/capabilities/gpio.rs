use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::apps::{GpioCapability, GpioMode, GpioOperation};

/// Hardware-independent GPIO operations. Implementations receive board aliases,
/// never raw chip or line numbers from an app.
pub trait GpioBackend: Send + Sync {
    fn configure(&self, alias: &str, mode: GpioMode, safe_value: Option<bool>) -> Result<()>;
    fn read(&self, alias: &str) -> Result<bool>;
    fn write(&self, alias: &str, value: bool) -> Result<()>;
    fn safe_reset(&self, alias: &str, safe_value: Option<bool>) -> Result<()>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpioRequest {
    pub alias: String,
    pub operation: GpioOperation,
    #[serde(default)]
    pub mode: Option<GpioMode>,
    #[serde(default)]
    pub value: Option<bool>,
}

/// Per-app authorization layer in front of the board backend.
pub struct GpioBroker {
    backend: Arc<dyn GpioBackend>,
    permissions: HashMap<String, GpioCapability>,
}

impl GpioBroker {
    pub fn new(backend: Arc<dyn GpioBackend>, capabilities: &[GpioCapability]) -> Result<Self> {
        let mut permissions = HashMap::new();
        for capability in capabilities {
            let alias = capability.alias.trim();
            if alias.is_empty() {
                bail!("GPIO capability alias cannot be empty");
            }
            if permissions
                .insert(alias.to_string(), capability.clone())
                .is_some()
            {
                bail!("duplicate GPIO capability alias: {alias}");
            }
        }
        Ok(Self {
            backend,
            permissions,
        })
    }

    pub fn execute(&self, request: &GpioRequest) -> Result<Option<bool>> {
        let capability = self
            .permissions
            .get(&request.alias)
            .ok_or_else(|| anyhow!("GPIO alias is not declared: {}", request.alias))?;
        if !capability.operations.contains(&request.operation) {
            bail!(
                "GPIO operation {:?} is not allowed for alias {}",
                request.operation,
                request.alias
            );
        }

        match request.operation {
            GpioOperation::Configure => {
                let mode = request
                    .mode
                    .ok_or_else(|| anyhow!("GPIO configure requires a mode"))?;
                self.backend
                    .configure(&request.alias, mode, capability.safe_value)?;
                Ok(None)
            }
            GpioOperation::Read => Ok(Some(self.backend.read(&request.alias)?)),
            GpioOperation::Write => {
                let value = request
                    .value
                    .ok_or_else(|| anyhow!("GPIO write requires a value"))?;
                self.backend.write(&request.alias, value)?;
                Ok(None)
            }
        }
    }

    /// Restore every declared alias, including aliases the app never configured.
    pub fn safe_reset(&self) -> Result<()> {
        let mut failures = Vec::new();
        for capability in self.permissions.values() {
            if let Err(error) = self
                .backend
                .safe_reset(&capability.alias, capability.safe_value)
            {
                failures.push(format!("{}: {error}", capability.alias));
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            bail!("GPIO safe reset failed: {}", failures.join("; "))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MockGpioPinState {
    pub configured: bool,
    pub mode: Option<GpioMode>,
    pub value: bool,
    pub safe_value: Option<bool>,
}

impl Default for MockGpioPinState {
    fn default() -> Self {
        Self {
            configured: false,
            mode: None,
            value: false,
            safe_value: None,
        }
    }
}

/// Deterministic development backend used on Windows and in unit tests.
#[derive(Default)]
pub struct MockGpioBackend {
    pins: Mutex<HashMap<String, MockGpioPinState>>,
}

impl MockGpioBackend {
    pub fn pin_state(&self, alias: &str) -> Result<Option<MockGpioPinState>> {
        Ok(self
            .pins
            .lock()
            .map_err(|_| anyhow!("mock GPIO lock poisoned"))?
            .get(alias)
            .cloned())
    }
}

impl GpioBackend for MockGpioBackend {
    fn configure(&self, alias: &str, mode: GpioMode, safe_value: Option<bool>) -> Result<()> {
        if alias.trim().is_empty() {
            bail!("GPIO alias cannot be empty");
        }
        let mut pins = self
            .pins
            .lock()
            .map_err(|_| anyhow!("mock GPIO lock poisoned"))?;
        let pin = pins.entry(alias.to_string()).or_default();
        pin.configured = true;
        pin.mode = Some(mode);
        pin.safe_value = safe_value;
        if let Some(value) = safe_value {
            pin.value = value;
        }
        Ok(())
    }

    fn read(&self, alias: &str) -> Result<bool> {
        let pins = self
            .pins
            .lock()
            .map_err(|_| anyhow!("mock GPIO lock poisoned"))?;
        let pin = pins
            .get(alias)
            .ok_or_else(|| anyhow!("GPIO alias is not configured: {alias}"))?;
        if !pin.configured {
            bail!("GPIO alias is not configured: {alias}");
        }
        Ok(pin.value)
    }

    fn write(&self, alias: &str, value: bool) -> Result<()> {
        let mut pins = self
            .pins
            .lock()
            .map_err(|_| anyhow!("mock GPIO lock poisoned"))?;
        let pin = pins
            .get_mut(alias)
            .ok_or_else(|| anyhow!("GPIO alias is not configured: {alias}"))?;
        if !pin.configured {
            bail!("GPIO alias is not configured: {alias}");
        }
        if pin.mode != Some(GpioMode::Output) {
            bail!("GPIO alias is not configured for output: {alias}");
        }
        pin.value = value;
        Ok(())
    }

    fn safe_reset(&self, alias: &str, safe_value: Option<bool>) -> Result<()> {
        if alias.trim().is_empty() {
            bail!("GPIO alias cannot be empty");
        }
        let mut pins = self
            .pins
            .lock()
            .map_err(|_| anyhow!("mock GPIO lock poisoned"))?;
        let pin = pins.entry(alias.to_string()).or_default();
        pin.value = safe_value.unwrap_or(false);
        pin.safe_value = safe_value;
        pin.configured = false;
        pin.mode = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn output_capability() -> GpioCapability {
        GpioCapability {
            alias: "status_led".to_string(),
            operations: vec![GpioOperation::Configure, GpioOperation::Write],
            safe_value: Some(false),
        }
    }

    #[test]
    fn broker_rejects_undeclared_alias_and_operation() {
        let backend = Arc::new(MockGpioBackend::default());
        let broker = GpioBroker::new(backend, &[output_capability()]).expect("broker");

        let undeclared = broker.execute(&GpioRequest {
            alias: "raw_pin_42".to_string(),
            operation: GpioOperation::Write,
            mode: None,
            value: Some(true),
        });
        assert!(undeclared
            .expect_err("undeclared alias must fail")
            .to_string()
            .contains("not declared"));

        let denied = broker.execute(&GpioRequest {
            alias: "status_led".to_string(),
            operation: GpioOperation::Read,
            mode: None,
            value: None,
        });
        assert!(denied
            .expect_err("undeclared operation must fail")
            .to_string()
            .contains("not allowed"));
    }

    #[test]
    fn mock_backend_restores_safe_value_on_reset() {
        let backend = Arc::new(MockGpioBackend::default());
        let broker = GpioBroker::new(backend.clone(), &[output_capability()]).expect("broker");

        broker
            .execute(&GpioRequest {
                alias: "status_led".to_string(),
                operation: GpioOperation::Configure,
                mode: Some(GpioMode::Output),
                value: None,
            })
            .expect("configure");
        broker
            .execute(&GpioRequest {
                alias: "status_led".to_string(),
                operation: GpioOperation::Write,
                mode: None,
                value: Some(true),
            })
            .expect("write");
        assert_eq!(
            backend
                .pin_state("status_led")
                .expect("state")
                .expect("pin")
                .value,
            true
        );

        broker.safe_reset().expect("safe reset");
        let reset = backend
            .pin_state("status_led")
            .expect("state")
            .expect("pin");
        assert!(!reset.value);
        assert!(!reset.configured);
        assert_eq!(reset.mode, None);
    }
}

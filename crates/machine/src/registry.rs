//! `DeviceRegistry`: the machine table (`machines.toml`, built in, plus any
//! user file of the same shape) and discovery — USB id lookup, firmware-string
//! probing over any transport, and manual selection by id.

use std::sync::LazyLock;

use serde::Deserialize;

use crate::{Capabilities, MachineError, Transport};

/// The built-in table, compiled into the binary.
pub const BUILTIN_MACHINES_TOML: &str = include_str!("../machines.toml");

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MachineFile {
    #[serde(default)]
    machine: Vec<Capabilities>,
}

/// A set of machines, searchable by id, USB id and firmware string.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DeviceRegistry {
    machines: Vec<Capabilities>,
}

static BUILTIN: LazyLock<Result<DeviceRegistry, MachineError>> =
    LazyLock::new(|| DeviceRegistry::from_toml(BUILTIN_MACHINES_TOML));

/// A USB device seen during enumeration (vendor/product id plus where it is).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsbIds {
    pub vendor_id: u16,
    pub product_id: u16,
    /// `bus-address`, stable for the life of the connection.
    pub location: String,
    pub product: Option<String>,
}

/// USB vendor ids worth enumerating for cutters (Graphtec/Silhouette, Cameo 5 Alpha).
pub const USB_VENDORS: &[u16] = &[0x0b4d, 0x3844];

impl DeviceRegistry {
    /// The built-in table. Never panics: a table that failed to parse (caught
    /// by the test suite) reads as empty, and `builtin_error` says why.
    pub fn builtin() -> &'static DeviceRegistry {
        static EMPTY: DeviceRegistry = DeviceRegistry {
            machines: Vec::new(),
        };
        BUILTIN.as_ref().unwrap_or(&EMPTY)
    }

    pub fn builtin_error() -> Option<&'static MachineError> {
        BUILTIN.as_ref().err()
    }

    /// Parse and check a machine table (unique ids, well-formed USB ids, sane ranges).
    pub fn from_toml(text: &str) -> Result<DeviceRegistry, MachineError> {
        let file: MachineFile = toml::from_str(text)
            .map_err(|e| MachineError::Config(format!("machine table: {e}")))?;
        let reg = DeviceRegistry {
            machines: file.machine,
        };
        reg.check()?;
        Ok(reg)
    }

    /// This registry plus a user table; entries with an existing id replace it.
    pub fn with_overrides(&self, text: &str) -> Result<DeviceRegistry, MachineError> {
        let extra = DeviceRegistry::from_toml(text)?;
        let mut machines = self.machines.clone();
        for m in extra.machines {
            match machines.iter_mut().find(|x| x.id == m.id) {
                Some(slot) => *slot = m,
                None => machines.push(m),
            }
        }
        let reg = DeviceRegistry { machines };
        reg.check()?;
        Ok(reg)
    }

    fn check(&self) -> Result<(), MachineError> {
        let bad = |m: String| Err(MachineError::Config(m));
        let mut ids = std::collections::HashSet::new();
        for m in &self.machines {
            if m.id.trim().is_empty() || !ids.insert(m.id.as_str()) {
                return bad(format!("machine id `{}` is empty or duplicated", m.id));
            }
            if m.usb.is_some() && m.usb_ids().is_none() {
                return bad(format!("{}: usb must be \"vvvv:pppp\" hex", m.id));
            }
            for r in [&m.force, &m.speed].into_iter().flatten() {
                if r.min > r.max {
                    return bad(format!("{}: range {}..{} is inverted", m.id, r.min, r.max));
                }
            }
            for v in [m.cut_width_mm, m.cut_length_mm, m.media_width_mm]
                .into_iter()
                .flatten()
            {
                if !v.is_finite() || v <= 0.0 {
                    return bad(format!("{}: dimensions must be positive", m.id));
                }
            }
            if m.tool_holders == Some(0) {
                return bad(format!("{}: tool_holders must be at least 1", m.id));
            }
        }
        Ok(())
    }

    pub fn machines(&self) -> &[Capabilities] {
        &self.machines
    }

    pub fn by_id(&self, id: &str) -> Option<&Capabilities> {
        let id = id.trim();
        self.machines.iter().find(|m| m.id.eq_ignore_ascii_case(id))
    }

    pub fn by_usb(&self, vendor_id: u16, product_id: u16) -> Option<&Capabilities> {
        self.machines
            .iter()
            .find(|m| m.usb_ids() == Some((vendor_id, product_id)))
    }

    /// Identify a firmware-version reply (`CAMEO 4 PRO V1.02`): the longest
    /// matching `firmware` needle wins, so `CAMEO 4 PLUS` never reads as `CAMEO 4`.
    pub fn by_firmware(&self, version: &str) -> Option<&Capabilities> {
        let upper = version.to_ascii_uppercase();
        self.machines
            .iter()
            .filter_map(|m| {
                let needle = m.firmware.as_deref()?.to_ascii_uppercase();
                upper.contains(&needle).then_some((needle.len(), m))
            })
            .max_by_key(|(len, _)| *len)
            .map(|(_, m)| m)
    }

    /// Keep the enumerated USB devices that are known machines.
    pub fn match_usb(
        &self,
        devices: impl IntoIterator<Item = UsbIds>,
    ) -> Vec<(UsbIds, &Capabilities)> {
        devices
            .into_iter()
            .filter_map(|d| self.by_usb(d.vendor_id, d.product_id).map(|m| (d, m)))
            .collect()
    }

    /// Probe an open port (serial, TCP, USB) with the read-only GP-GL
    /// firmware query and identify the machine. Sends `ESC EOT` (initialize)
    /// and `FG`; never moves the head.
    pub fn probe(
        &self,
        transport: &mut dyn Transport,
    ) -> Result<Option<&Capabilities>, MachineError> {
        transport.write(crate::ESC_INIT.as_bytes())?;
        let version = crate::query_version(transport)?;
        Ok(self.by_firmware(&version))
    }
}

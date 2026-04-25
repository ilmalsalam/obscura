#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleState {
    Idle,
    Loading,
    DomContentLoaded,
    Loaded,
    NetworkIdle,
    Failed,
}

impl LifecycleState {
    pub fn is_loading(&self) -> bool {
        matches!(self, LifecycleState::Loading)
    }

    pub fn is_loaded(&self) -> bool {
        matches!(self, LifecycleState::Loaded | LifecycleState::NetworkIdle)
    }

    pub fn is_network_idle(&self) -> bool {
        matches!(self, LifecycleState::NetworkIdle)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitUntil {
    Load,
    DomContentLoaded,
    NetworkIdle0,
    NetworkIdle2,
    /// Wait for JavaScript SPA hydration to complete.
    /// This extends the load phase with extended JS event loop polling
    /// to allow React/Vue/Angular apps to fully render.
    Hydration,
}

impl WaitUntil {
    pub fn from_str(s: &str) -> Self {
        match s {
            "domcontentloaded" => WaitUntil::DomContentLoaded,
            "networkidle0" | "networkIdle" => WaitUntil::NetworkIdle0,
            "networkidle2" => WaitUntil::NetworkIdle2,
            "hydration" | "spa" => WaitUntil::Hydration,
            _ => WaitUntil::Load,
        }
    }

    /// Returns true if this wait type requires extended JS event loop polling
    /// for SPA/React hydration
    pub fn requires_hydration_wait(&self) -> bool {
        matches!(self, WaitUntil::Hydration)
    }
}

use forge_core::PermissionSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    FileRead,
    FileWrite,
    NetConnect,
    ProcessSpawn,
}

pub struct PermissionChecker {
    permissions: PermissionSet,
}

impl PermissionChecker {
    pub fn new(permissions: PermissionSet) -> Self {
        Self { permissions }
    }

    pub fn check_file(&self, path: &str, write: bool) -> bool {
        self.permissions.filesystem.iter().any(|p| {
            path.starts_with(&p.path) && p.read && (!write || p.write)
        })
    }

    pub fn check_network(&self, host: &str, port: u16) -> bool {
        self.permissions.network.iter().any(|p| {
            p.host == host && (p.ports.is_empty() || p.ports.contains(&port))
        })
    }

    pub fn check_process(&self, command: &str) -> bool {
        self.permissions.process.iter().any(|p| p.command == command)
    }

    pub fn check(&self, action: Action, target: &str) -> bool {
        match action {
            Action::FileRead => self.check_file(target, false),
            Action::FileWrite => self.check_file(target, true),
            Action::NetConnect => {
                // Parse "host:port"
                if let Some((host, port_str)) = target.rsplit_once(':') {
                    if let Ok(port) = port_str.parse::<u16>() {
                        return self.check_network(host, port);
                    }
                }
                false
            }
            Action::ProcessSpawn => self.check_process(target),
        }
    }
}

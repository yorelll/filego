//! Transactional launch-at-login setting over an injectable OS registry seam.
//!
//! Registry writes are external effects. Persisting the corresponding JSON
//! setting must succeed first; a failed registry write restores the previous
//! setting. No data document means no registry mutation whatsoever.

use super::settings_controller::{SNotice, SettingsController, SettingsStore};

pub trait RunRegistration {
    type Error;

    fn read_enabled(&mut self) -> Result<bool, Self::Error>;
    fn write_enabled(&mut self, enabled: bool) -> Result<(), Self::Error>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistrationError {
    DataUnavailable,
    SaveFailed,
    ReadFailed,
    WriteFailed,
    RollbackFailed,
}

impl RegistrationError {
    pub const fn notice(self) -> SNotice {
        match self {
            Self::ReadFailed => SNotice::StartupReadFailed,
            Self::DataUnavailable | Self::SaveFailed | Self::WriteFailed | Self::RollbackFailed => {
                SNotice::StartupWriteFailed
            }
        }
    }
}

/// Change both persisted JSON and HKCU Run, never claiming success if either
/// fails. We reject a mismatched pre-existing OS registration rather than
/// silently treating an unowned stale value as ours.
pub fn set_launch_at_login<S: SettingsStore, R: RunRegistration>(
    controller: &mut SettingsController<S>,
    registry: &mut R,
    enabled: bool,
) -> Result<(), RegistrationError> {
    let Some(document) = controller.document() else {
        controller.set_notice(SNotice::SaveFailed);
        return Err(RegistrationError::DataUnavailable);
    };
    let prior_settings = document.data.settings;
    let prior_os = registry.read_enabled().map_err(|_| {
        controller.set_notice(SNotice::StartupReadFailed);
        RegistrationError::ReadFailed
    })?;
    if enabled == prior_settings.launch_at_login && enabled == prior_os {
        controller.set_launch_at_login_os(Some(prior_os));
        return Ok(());
    }
    controller
        .set_launch_at_login(enabled)
        .map_err(|_| RegistrationError::SaveFailed)?;
    let write_ok = registry.write_enabled(enabled).is_ok();
    let verified = registry.read_enabled();
    if write_ok && matches!(verified, Ok(state) if state == enabled) {
        controller.set_launch_at_login_os(Some(enabled));
        return Ok(());
    }

    // Always try BOTH rollbacks, even when one fails. A failed Win32 call may
    // still have partially changed HKCU; never assume an Err means no effect.
    let os_rollback = registry.write_enabled(prior_os);
    let data_rollback = controller.set_launch_at_login(prior_settings.launch_at_login);
    let actual_os = registry.read_enabled().ok();
    controller.set_launch_at_login_os(actual_os);
    if os_rollback.is_err() || data_rollback.is_err() || actual_os != Some(prior_os) {
        controller.set_notice(SNotice::StartupWriteFailed);
        return Err(RegistrationError::RollbackFailed);
    }
    let error = if write_ok && verified.is_err() {
        RegistrationError::ReadFailed
    } else {
        RegistrationError::WriteFailed
    };
    controller.set_notice(error.notice());
    Err(error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::settings_controller::DocumentError;
    use crate::{
        domain::{document::AppData, settings::AppSettings},
        storage::{repository::RepositoryError, schema::StoredDocumentV1},
    };

    struct Store {
        data: Option<StoredDocumentV1>,
        fail_save: bool,
        fail_on: Option<bool>,
    }

    impl SettingsStore for Store {
        fn load_document(&mut self) -> Result<StoredDocumentV1, DocumentError> {
            self.data.clone().ok_or(RepositoryError::NotFound)
        }
        fn document(&self) -> Option<StoredDocumentV1> {
            self.data.clone()
        }
        fn set_settings(&mut self, settings: AppSettings) -> Result<(), DocumentError> {
            self.data
                .as_mut()
                .ok_or(RepositoryError::NotFound)?
                .data
                .settings = settings;
            Ok(())
        }
        fn set_data(&mut self, data: AppData) -> Result<(), DocumentError> {
            self.data.as_mut().ok_or(RepositoryError::NotFound)?.data = data;
            Ok(())
        }
        fn clear_folder_records(&mut self) -> bool {
            false
        }
        fn rename_category_record(
            &mut self,
            _: crate::domain::ids::CategoryId,
            _: String,
        ) -> Result<(), DocumentError> {
            Err(RepositoryError::NotFound)
        }
        fn rename_tag_record(
            &mut self,
            _: crate::domain::ids::TagId,
            _: String,
        ) -> Result<(), DocumentError> {
            Err(RepositoryError::NotFound)
        }
        fn merge_tag_record(
            &mut self,
            _: crate::domain::ids::TagId,
            _: crate::domain::ids::TagId,
        ) -> Result<(), DocumentError> {
            Err(RepositoryError::NotFound)
        }
        fn save_at(&mut self) -> Result<u64, DocumentError> {
            if self.fail_save
                || self.fail_on == self.data.as_ref().map(|d| d.data.settings.launch_at_login)
            {
                return Err(RepositoryError::Io);
            }
            Ok(2)
        }
    }

    struct Registry {
        state: bool,
        writes: usize,
        fail_read: bool,
        fail_write: bool,
        fail_first_write: bool,
        fail_read_after_write: bool,
        partial_write: bool,
    }
    impl RunRegistration for Registry {
        type Error = ();
        fn read_enabled(&mut self) -> Result<bool, Self::Error> {
            if self.fail_read || (self.fail_read_after_write && self.writes == 1) {
                Err(())
            } else {
                Ok(self.state)
            }
        }
        fn write_enabled(&mut self, enabled: bool) -> Result<(), Self::Error> {
            self.writes += 1;
            if self.partial_write && self.writes == 1 {
                self.state = enabled;
                return Err(());
            }
            if self.fail_write || (self.fail_first_write && self.writes == 1) {
                Err(())
            } else {
                self.state = enabled;
                Ok(())
            }
        }
    }

    fn controller(
        with_data: bool,
        fail_save: bool,
        fail_on: Option<bool>,
    ) -> SettingsController<Store> {
        SettingsController::new(Store {
            data: with_data.then(|| {
                StoredDocumentV1::new(AppData {
                    settings: AppSettings::default(),
                    folders: vec![],
                    categories: vec![],
                    tags: vec![],
                    revision: 1,
                })
            }),
            fail_save,
            fail_on,
        })
    }
    fn registry() -> Registry {
        Registry {
            state: false,
            writes: 0,
            fail_read: false,
            fail_write: false,
            fail_first_write: false,
            fail_read_after_write: false,
            partial_write: false,
        }
    }

    #[test]
    fn no_document_never_touches_registry() {
        let mut settings = controller(false, false, None);
        let mut os = registry();
        assert_eq!(
            set_launch_at_login(&mut settings, &mut os, true),
            Err(RegistrationError::DataUnavailable)
        );
        assert_eq!(os.writes, 0);
        assert!(!os.state);
    }

    #[test]
    fn pending_repair_repository_never_changes_registration() {
        use crate::presentation::settings_controller::SettingsSharedStore;
        use crate::storage::{codec, location::DocumentPaths, repository::DocumentRepository};
        let root = tempfile::tempdir().expect("temporary data directory");
        let valid = StoredDocumentV1::new(AppData {
            settings: AppSettings::default(),
            folders: vec![],
            categories: vec![],
            tags: vec![],
            revision: 1,
        });
        let corrupt = b"corrupt document";
        std::fs::write(root.path().join("data.json"), corrupt).expect("corrupt main");
        std::fs::write(
            root.path().join("data.json.bak"),
            codec::encode(&valid).expect("encode"),
        )
        .expect("internal backup");
        let mut repo = DocumentRepository::new(DocumentPaths::from_base_dir(root.path()));
        assert!(matches!(
            repo.load(),
            Ok(crate::storage::repository::LoadOutcome::Recovered(_))
        ));
        let shared = std::rc::Rc::new(std::cell::RefCell::new(repo));
        let mut settings = SettingsController::new(SettingsSharedStore::new(shared));
        let mut os = registry();
        assert_eq!(
            set_launch_at_login(&mut settings, &mut os, true),
            Err(RegistrationError::SaveFailed)
        );
        assert_eq!(os.writes, 0);
        assert!(!settings.document().unwrap().data.settings.launch_at_login);
        assert_eq!(
            std::fs::read(root.path().join("data.json")).expect("corrupt main intact"),
            corrupt
        );
    }

    #[test]
    fn toggle_enable_disable_is_persisted_and_idempotent() {
        let mut settings = controller(true, false, None);
        let mut os = registry();
        set_launch_at_login(&mut settings, &mut os, true).expect("enable");
        assert!(os.state);
        assert!(settings.document().unwrap().data.settings.launch_at_login);
        set_launch_at_login(&mut settings, &mut os, true).expect("repeat enable");
        assert_eq!(os.writes, 1);
        set_launch_at_login(&mut settings, &mut os, false).expect("disable");
        assert!(!os.state);
    }

    #[test]
    fn failed_registry_write_rolls_back_json() {
        let mut settings = controller(true, false, None);
        let mut os = registry();
        os.fail_first_write = true;
        assert_eq!(
            set_launch_at_login(&mut settings, &mut os, true),
            Err(RegistrationError::WriteFailed)
        );
        assert_eq!(os.writes, 2, "the failed call still requires OS rollback");
        assert!(!os.state);
        assert!(!settings.document().unwrap().data.settings.launch_at_login);
    }

    #[test]
    fn failed_persist_prevents_registry_writes() {
        let mut settings = controller(true, true, None);
        let mut os = registry();
        assert_eq!(
            set_launch_at_login(&mut settings, &mut os, true),
            Err(RegistrationError::SaveFailed)
        );
        assert_eq!(os.writes, 0);
        assert!(!settings.document().unwrap().data.settings.launch_at_login);
    }

    #[test]
    fn failed_registry_rollback_surfaces_anonymous_error() {
        let mut settings = controller(true, false, Some(false));
        let mut os = registry();
        os.fail_write = true;
        assert_eq!(
            set_launch_at_login(&mut settings, &mut os, true),
            Err(RegistrationError::RollbackFailed)
        );
    }

    #[test]
    fn failed_registry_write_after_mutation_restores_both_states() {
        let mut settings = controller(true, false, None);
        let mut os = registry();
        os.partial_write = true;
        assert_eq!(
            set_launch_at_login(&mut settings, &mut os, true),
            Err(RegistrationError::WriteFailed)
        );
        assert!(!os.state);
        assert!(!settings.document().unwrap().data.settings.launch_at_login);
    }

    #[test]
    fn failed_verification_rolls_back_registry_and_document_independently() {
        let mut settings = controller(true, false, Some(false));
        let mut os = registry();
        os.fail_read_after_write = true;
        assert_eq!(
            set_launch_at_login(&mut settings, &mut os, true),
            Err(RegistrationError::RollbackFailed)
        );
        assert_eq!(
            os.writes, 2,
            "JSON rollback failure must not skip OS rollback"
        );
        assert!(!os.state);
    }

    #[test]
    fn failed_verification_with_successful_rollback_is_read_error() {
        let mut settings = controller(true, false, None);
        let mut os = registry();
        os.fail_read_after_write = true;
        assert_eq!(
            set_launch_at_login(&mut settings, &mut os, true),
            Err(RegistrationError::ReadFailed)
        );
        assert!(!os.state);
        assert!(!settings.document().unwrap().data.settings.launch_at_login);
    }

    #[test]
    fn failed_registry_read_never_writes() {
        let mut settings = controller(true, false, None);
        let mut os = registry();
        os.fail_read = true;
        assert_eq!(
            set_launch_at_login(&mut settings, &mut os, true),
            Err(RegistrationError::ReadFailed)
        );
        assert_eq!(os.writes, 0);
    }
}

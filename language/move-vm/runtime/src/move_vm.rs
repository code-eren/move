// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeSet, sync::Arc};

use crate::{
    data_cache::TransactionDataCache, native_extensions::NativeContextExtensions,
    native_functions::NativeFunction, runtime::VMRuntime, session::Session,
};
use move_binary_format::{
    errors::{Location, VMResult},
    CompiledModule,
};
use move_core_types::{
    account_address::AccountAddress, identifier::Identifier, language_storage::ModuleId,
    resolver::MoveResolver,
};

pub struct MoveVM {
    runtime: VMRuntime,
}

impl MoveVM {
    pub fn new(
        natives: impl IntoIterator<Item = (AccountAddress, Identifier, Identifier, NativeFunction)>,
    ) -> VMResult<Self> {
        Ok(Self {
            runtime: VMRuntime::new(natives).map_err(|err| err.finish(Location::Undefined))?,
        })
    }

    /// Create a new Session backed by the given storage.
    ///
    /// Right now it is the caller's responsibility to ensure cache coherence of the Move VM Loader
    ///   - When a module gets published in a Move VM Session, and then gets used by another
    ///     transaction, it will be loaded into the code cache and stay there even if the resulted
    ///     effects do not get commited back to the storage when the Session ends.
    ///   - As a result, if one wants to have multiple sessions at a time, one needs to make sure
    ///     none of them will try to publish a module. In other words, if there is a module publishing
    ///     Session it must be the only Session existing.
    ///   - In general, a new Move VM needs to be created whenever the storage gets modified by an
    ///     outer envrionment, or otherwise the states may be out of sync. There are a few exceptional
    ///     cases where this may not be necessary, with the most notable one being the common module
    ///     publishing flow: you can keep using the same Move VM if you publish some modules in a Session
    ///     and apply the effects to the storage when the Session ends.
    pub fn new_session<'r, S: MoveResolver>(&self, remote: &'r S) -> Session<'r, '_, S> {
        self.runtime.new_session(remote)
    }

    /// Create a new session, as in `new_session`, but provide native context extensions.
    pub fn new_session_with_extensions<'r, S: MoveResolver>(
        &self,
        remote: &'r S,
        extensions: NativeContextExtensions<'r>,
    ) -> Session<'r, '_, S> {
        self.runtime.new_session_with_extensions(remote, extensions)
    }

    /// Load a module into VM's code cache
    pub fn load_module<'r, S: MoveResolver>(
        &self,
        module_id: &ModuleId,
        remote: &'r S,
    ) -> VMResult<Arc<CompiledModule>> {
        self.runtime
            .loader()
            .load_module(
                module_id,
                &TransactionDataCache::new(remote, self.runtime.loader()),
            )
            .map(|arc_module| arc_module.arc_module())
    }

    /// Allows the adapter to announce to the VM that the code loading cache should be considered
    /// outdated. This can happen if the adapter executed a particular code publishing transaction
    /// but decided to not commit the result to the data store. Because the code cache currently
    /// does not support deletion, the cache will, incorrectly, still contain this module.
    /// TODO: new loader architecture
    pub fn mark_loader_cache_as_invalid(&self) {
        self.runtime.loader().mark_as_invalid()
    }

    /// If the loader cache has been invalidated (either by the above call or by internal logic)
    /// flush it so it is valid again. Notice that should only be called if there are no
    /// outstanding sessions created from this VM.
    /// TODO: new loader architecture
    pub fn flush_loader_cache_if_invalidated(&self) {
        self.runtime.loader().flush_if_invalidated()
    }

    /// Gets and clears module cache hits. This is hack which allows the adapter to see module
    /// reads if executing multiple transactions in a VM. Without this, the adapter only sees
    /// the first load of a module.
    /// TODO: new loader architecture
    pub fn get_and_clear_module_cache_hits(&self) -> BTreeSet<ModuleId> {
        self.runtime.loader().get_and_clear_module_cache_hits()
    }
}

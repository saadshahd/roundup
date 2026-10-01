//! Pads: markdown notes, app-stored or as files. Owner: pads Builder.

mod files;
mod name;
mod store;

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use contracts::pad::{
    AppendParams, CreateParams, ExportParams, Pad, PadName, SetOwnerParams, SetStorageParams,
    WriteParams,
};
use contracts::{ActorKind, EventData, Verb};
use rpc::{Bus, Ctx, Module, OpenError, RpcError, params, reply};
use serde_json::Value;

use store::Store;

pub struct Pads {
    store: Mutex<Store>,
    files_dir: PathBuf,
}

impl Pads {
    /// `dir` is the Project's `.roundup/` directory. `bus` is for events no call caused.
    pub fn open(dir: &Path, _bus: Bus) -> Result<Self, OpenError> {
        Ok(Self {
            store: Mutex::new(Store::open(&dir.join("pads.db"))?),
            files_dir: dir.join("pads"),
        })
    }

    fn create(&self, ctx: &Ctx, p: CreateParams) -> Result<Value, RpcError> {
        name::validate(&p.name)?;
        let pad = Pad {
            name: p.name,
            owner: ctx.actor.clone(),
            text: p.text.unwrap_or_default(),
            updated_at: now_ms(),
        };
        let store = self.store()?;
        if !store.insert(&pad).map_err(RpcError::internal)? {
            return Err(RpcError::conflict(format!("pad exists: {}", pad.name)));
        }
        self.mirror(&store, &pad)?;
        wrote(ctx, &pad.name)?;
        reply(&pad)
    }

    fn read(&self, ctx: &Ctx, p: PadName) -> Result<Value, RpcError> {
        let store = self.store()?;
        let pad = get(&store, &p.name)?;
        ctx.touch(Verb::Read, &item(&pad.name))?;
        reply(&pad)
    }

    fn list(&self) -> Result<Value, RpcError> {
        reply(&self.store()?.list().map_err(RpcError::internal)?)
    }

    fn write(&self, ctx: &Ctx, p: WriteParams) -> Result<Value, RpcError> {
        let store = self.store()?;
        let pad = get(&store, &p.name)?;
        if pad.owner != ctx.actor {
            return Err(RpcError::forbidden(format!(
                "only {} may rewrite pad {}",
                pad.owner.id, pad.name
            )));
        }
        store
            .set_text(&pad.name, &p.text, now_ms())
            .map_err(RpcError::internal)?;
        wrote(ctx, &pad.name)?;
        reply(&Pad {
            text: p.text,
            ..pad
        })
    }

    fn append(&self, ctx: &Ctx, p: AppendParams) -> Result<Value, RpcError> {
        let store = self.store()?;
        let pad = get(&store, &p.name)?;
        let text = pad.text + &p.text;
        store
            .set_text(&pad.name, &text, now_ms())
            .map_err(RpcError::internal)?;
        let pad = Pad { text, ..pad };
        self.mirror(&store, &pad)?;
        wrote(ctx, &pad.name)?;
        reply(&pad)
    }

    fn set_owner(&self, ctx: &Ctx, p: SetOwnerParams) -> Result<Value, RpcError> {
        let store = self.store()?;
        let pad = get(&store, &p.name)?;
        require_owner_or_user(ctx, &pad, "hand over")?;
        store
            .set_owner(&pad.name, &p.owner, now_ms())
            .map_err(RpcError::internal)?;
        wrote(ctx, &pad.name)?;
        reply(&Pad {
            owner: p.owner,
            ..pad
        })
    }

    fn delete(&self, ctx: &Ctx, p: PadName) -> Result<Value, RpcError> {
        let store = self.store()?;
        let pad = get(&store, &p.name)?;
        require_owner_or_user(ctx, &pad, "delete")?;
        store.delete(&pad.name).map_err(RpcError::internal)?;
        wrote(ctx, &pad.name)?;
        Ok(Value::Null)
    }

    fn export(&self, ctx: &Ctx, p: ExportParams) -> Result<Value, RpcError> {
        let store = self.store()?;
        let pad = get(&store, &p.name)?;
        std::fs::write(&p.path, &pad.text).map_err(|err| {
            let code = match err.kind() {
                std::io::ErrorKind::NotFound => rpc::code::INVALID_PARAMS,
                _ => rpc::code::INTERNAL,
            };
            RpcError::new(code, format!("cannot export to {}: {err}", p.path))
        })?;
        ctx.touch(Verb::Read, &item(&pad.name))?;
        Ok(Value::Null)
    }

    fn set_storage(&self, ctx: &Ctx, p: SetStorageParams) -> Result<Value, RpcError> {
        let store = self.store()?;
        if store.files().map_err(RpcError::internal)? == p.files {
            return Ok(Value::Null);
        }
        store.set_files(p.files).map_err(RpcError::internal)?;
        for pad in store.list().map_err(RpcError::internal)? {
            if p.files {
                self.mirror(&store, &pad)?;
            } else if let Some(text) =
                files::read(&self.files_dir, &pad.name).map_err(RpcError::internal)?
                && text != pad.text
            {
                store
                    .set_text(&pad.name, &text, now_ms())
                    .map_err(RpcError::internal)?;
                wrote(ctx, &pad.name)?;
            }
        }
        Ok(Value::Null)
    }

    /// Keeps the file in step with the Pad while file storage is on.
    fn mirror(&self, store: &Store, pad: &Pad) -> Result<(), RpcError> {
        if store.files().map_err(RpcError::internal)? {
            files::write(&self.files_dir, &pad.name, &pad.text).map_err(RpcError::internal)?;
        }
        Ok(())
    }

    fn store(&self) -> Result<std::sync::MutexGuard<'_, Store>, RpcError> {
        self.store
            .lock()
            .map_err(|_| RpcError::internal("pad store poisoned"))
    }
}

#[async_trait]
impl Module for Pads {
    fn namespaces(&self) -> &'static [&'static str] {
        &["pad"]
    }

    async fn call(&self, ctx: &Ctx, method: &str, value: Value) -> Result<Value, RpcError> {
        match method {
            "pad.create" => self.create(ctx, params(value)?),
            "pad.read" => self.read(ctx, params(value)?),
            "pad.list" => self.list(),
            "pad.write" => self.write(ctx, params(value)?),
            "pad.append" => self.append(ctx, params(value)?),
            "pad.setOwner" => self.set_owner(ctx, params(value)?),
            "pad.delete" => self.delete(ctx, params(value)?),
            "pad.setStorage" => self.set_storage(ctx, params(value)?),
            "pad.export" => self.export(ctx, params(value)?),
            _ => Err(RpcError::method_not_found(method)),
        }
    }
}

fn require_owner_or_user(ctx: &Ctx, pad: &Pad, action: &str) -> Result<(), RpcError> {
    if ctx.actor.kind == ActorKind::User || ctx.actor == pad.owner {
        return Ok(());
    }
    Err(RpcError::forbidden(format!(
        "only the user or {} may {action} pad {}",
        pad.owner.id, pad.name
    )))
}

/// Validates the name, then looks the Pad up.
fn get(store: &Store, name: &str) -> Result<Pad, RpcError> {
    name::validate(name)?;
    store
        .get(name)
        .map_err(RpcError::internal)?
        .ok_or_else(|| RpcError::not_found(format!("pad {name}")))
}

fn wrote(ctx: &Ctx, name: &str) -> Result<(), RpcError> {
    ctx.touch(Verb::Wrote, &item(name))?;
    ctx.emit(EventData::PadChanged(PadName { name: name.into() }));
    Ok(())
}

fn item(name: &str) -> String {
    format!("pad:{name}")
}

fn now_ms() -> i64 {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
    )
    .unwrap_or(i64::MAX)
}

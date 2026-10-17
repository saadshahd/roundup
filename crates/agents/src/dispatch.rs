use super::*;

#[async_trait]
impl Module for Agents {
    fn namespaces(&self) -> &'static [&'static str] {
        &["agent", "rail", "project", "decision", "models"]
    }

    async fn call(&self, ctx: &Ctx, method: &str, value: Value) -> Result<Value, RpcError> {
        let shared = &self.shared;
        match method {
            "agent.spawn" => reply(&self.spawn(ctx, params(value)?).await?),
            "agent.extras" => reply(&claude_code::extras::available(
                &self.launcher.extras_sources(self.project_dir()),
            )),
            "agent.extras_add" => {
                let params: ExtrasAddParams = params(value)?;
                let id = params.id.clone();
                let node = self.add_extras(ctx, params).await?;
                Self::touch_rail(ctx, &id)?;
                reply(&node)
            }
            "agent.prompt" => {
                let PromptParams { id, text } = params(value)?;
                self.may_steer(&ctx.actor, &id, "Steer")?;
                self.prompt(&id, &text).await?;
                Self::touch_rail(ctx, &id)?;
                reply(&())
            }
            "agent.interrupt" => {
                let NodeId { id } = params(value)?;
                self.may_steer(&ctx.actor, &id, "Interrupt")?;
                shared.interrupt(&id, None).await?;
                Self::touch_rail(ctx, &id)?;
                reply(&())
            }
            "agent.stop" => {
                let NodeId { id } = params(value)?;
                shared.guard_reach(&ctx.actor, &id, false)?;
                shared.stop(ctx.actor.clone(), &id).await?;
                Self::touch_rail(ctx, &id)?;
                reply(&())
            }
            "agent.worktreeState" => {
                let NodeId { id } = params(value)?;
                shared.guard_reach(&ctx.actor, &id, true)?;
                reply(&self.worktree_state(&id).await?)
            }
            "agent.land" => {
                let NodeId { id } = params(value)?;
                shared.guard_reach(&ctx.actor, &id, false)?;
                let landed = self.land(&id).await?;
                Self::touch_rail(ctx, &id)?;
                reply(&landed)
            }
            "agent.discard" => {
                let NodeId { id } = params(value)?;
                shared.guard_reach(&ctx.actor, &id, false)?;
                shared.discard(ctx.actor.clone(), &id).await?;
                Self::touch_rail(ctx, &id)?;
                reply(&())
            }
            "rail.startDoor" => {
                let start: StartDoorParams = params(value)?;
                let id = start.id.clone();
                shared.guard_reach(&ctx.actor, &id, false)?;
                let door = self.start_door(ctx, start).await?;
                Self::touch_rail(ctx, &id)?;
                reply(&door)
            }
            "agent.resume" => {
                let NodeId { id } = params(value)?;
                shared.guard_reach(&ctx.actor, &id, false)?;
                let node = self.resume(ctx, &id).await?;
                Self::touch_rail(ctx, &id)?;
                reply(&node)
            }
            "agent.setModel" => {
                let set: SetModelParams = params(value)?;
                let id = set.id.clone();
                let node = self.set_model(ctx, set).await?;
                Self::touch_rail(ctx, &id)?;
                reply(&node)
            }
            "agent.setOrder" => reply(&self.set_order(ctx, params(value)?).await?),
            "agent.channelUp" => {
                let NodeId { id } = params(value)?;
                shared.channel_up(&ctx.actor, &id)?;
                reply(&())
            }
            "agent.signal" => {
                let SignalParams {
                    id,
                    attempt,
                    payload,
                } = params(value)?;
                if contracts::agent::parse_positive_ordinal(&attempt).is_none() {
                    return Err(RpcError::new(
                        code::INVALID_PARAMS,
                        "attempt must be a canonical positive decimal within signed 64-bit range",
                    ));
                }
                // B39: the Signal H11 waits for confirms the Messages its prompt carried, before
                // the wait ends, so nothing can type them again in between. A Steer's text is
                // one paste, so a paste's envelope is taken off first.
                let drop = match (
                    claude_code::submitted_prompt(&payload),
                    shared.events.prompt_observer.get(),
                ) {
                    (Some(prompt), Some(observer)) => {
                        observer(&id, claude_code::pasted_text(prompt))
                    }
                    _ => false,
                };
                // The hook blocks a dropped prompt, so no turn starts and nothing is folded.
                if !drop {
                    shared.events.inbox.scan(&id, &payload);
                    shared.observe(
                        ctx.actor.clone(),
                        &id,
                        &attempt,
                        Observation::Signal(payload),
                    )?;
                }
                reply(&drop.then_some(SignalOutcome { drop }))
            }
            "agent.permission" => {
                let PermissionParams { id, payload } = params(value)?;
                reply(&shared.permission(ctx.actor.clone(), &id, payload).await?)
            }
            "agent.ask" => {
                let ask: contracts::decision::AskCall = params(value)?;
                reply(&shared.ask(ctx.actor.clone(), ask).await?)
            }
            "decision.answers" => {
                let contracts::decision::AnswersParams { agent } = params(value)?;
                reply(&shared.answers_for(&ctx.actor, &agent)?)
            }
            "decision.list" => reply(&shared.decisions_for(&ctx.actor)?),
            "decision.capture" => {
                let CaptureParams { id } = params(value)?;
                reply(&shared.capture(&ctx.actor, &id)?)
            }
            "decision.answer" => {
                let answer: AnswerParams = params(value)?;
                let shared = Arc::clone(shared);
                let ctx = Ctx {
                    actor: ctx.actor.clone(),
                    bus: ctx.bus.clone(),
                    touches: Arc::clone(&ctx.touches),
                };
                // An answer may wait for a guarded physical write. Keep that wait off
                // the reactor so the user's competing call can reach its own effect.
                tokio::task::spawn_blocking(move || shared.answer(&ctx, answer))
                    .await
                    .map_err(|err| {
                        RpcError::unknown_outcome(format!("prompt effect worker ended: {err}"))
                    })??;
                reply(&())
            }
            "rail.tree" => reply(&shared.tree()?),
            "agent.spawnPrompt" => {
                let NodeId { id } = params(value)?;
                shared.guard_reach(&ctx.actor, &id, true)?;
                reply(&serde_json::json!({ "prompt": shared.rail().spawn_prompt(&id)? }))
            }
            "rail.createWorkstream" => {
                let CreateWorkstreamParams {
                    name,
                    parent,
                    order,
                } = params(value)?;
                if let Some(order) = &order {
                    valid_explicit_order(order)?;
                }
                let node = shared.rail().insert_ordered(
                    NodeKind::Workstream,
                    &name,
                    parent.as_deref(),
                    None,
                    order.as_ref(),
                )?;
                if ctx.actor.kind == ActorKind::Agent {
                    shared.rail().set_creator(&node.id, &ctx.actor.id)?;
                }
                Self::touch_rail(ctx, &node.id)?;
                ctx.emit(EventData::RailChanged);
                reply(&shared.node(&node.id)?)
            }
            "rail.rename" => {
                let RenameParams { id, name } = params(value)?;
                shared.guard_reach(&ctx.actor, &id, true)?;
                let node = {
                    let mut rail = shared.rail();
                    let node = rail.rename(&id, &name)?;
                    if let Some(slot) = shared.runs().get_mut(&id) {
                        slot.settle_name();
                    }
                    node
                };
                Self::touch_rail(ctx, &id)?;
                ctx.emit(EventData::RailChanged);
                reply(&shared.node(&node.id)?)
            }
            "agent.run" => reply(&self.run(ctx, params(value)?).await?),
            "terminal.open" => reply(&self.open_terminal(params(value)?).await?),
            "project.setWorktrees" => {
                let Worktrees { on, check } = params(value)?;
                if check.as_deref() == Some("") {
                    return Err(RpcError::new(
                        code::INVALID_PARAMS,
                        "check must not be empty",
                    ));
                }
                shared.rail().set_worktrees(&Worktrees { on, check })?;
                reply(&())
            }
            "project.setMcp" => {
                let mcp: Mcp = params(value)?;
                shared.rail().set_mcp(&mcp)?;
                reply(&())
            }
            "project.setModels" => {
                let settings: ProjectModels = params(value)?;
                models::check_settings(&settings)?;
                shared.rail().set_models(&settings)?;
                ctx.touch(contracts::Verb::Wrote, "project:models")?;
                reply(&())
            }
            "models.list" => {
                ctx.touch(contracts::Verb::Read, "project:models")?;
                reply(&models::list())
            }
            "project.get" => {
                let worktrees = shared.rail().get_worktrees()?;
                let models = self.project_models()?;
                let mcp = shared.rail().get_mcp()?;
                reply(&ProjectSettings {
                    worktrees,
                    models,
                    mcp,
                })
            }
            "project.open" => {
                let contracts::project::OpenParams { path } = params(value)?;
                let folder = Path::new(&path);
                if !folder.is_absolute() {
                    return Err(RpcError::new(
                        code::INVALID_PARAMS,
                        format!("path {path} is not absolute"),
                    ));
                }
                if !folder.is_dir() {
                    return Err(RpcError::not_found(format!("directory {path}")));
                }
                ctx.touch(contracts::Verb::Wrote, &format!("project:{path}"))?;
                ctx.emit(EventData::ProjectRequested(
                    contracts::project::Requested::Open { path },
                ));
                reply(&())
            }
            "project.rename" => {
                let contracts::project::RenameParams { name } = params(value)?;
                let name = name.trim();
                if name.is_empty() || name.chars().count() > 40 || name.contains(['\n', '\r']) {
                    return Err(RpcError::new(
                        code::INVALID_PARAMS,
                        "a Project's name is 1 to 40 characters on one line",
                    ));
                }
                let path = shared.project_dir.to_string_lossy().into_owned();
                ctx.touch(contracts::Verb::Wrote, &format!("project:{path}"))?;
                ctx.emit(EventData::ProjectRequested(
                    contracts::project::Requested::Rename {
                        path,
                        name: name.to_owned(),
                    },
                ));
                reply(&())
            }
            "rail.move" => {
                let MoveParams { id, parent, index } = params(value)?;
                shared.guard_reach(&ctx.actor, &id, false)?;
                shared.rail().move_node(&id, parent.as_deref(), index)?;
                Self::touch_rail(ctx, &id)?;
                ctx.emit(EventData::RailChanged);
                reply(&())
            }
            "rail.remove" => {
                let NodeId { id } = params(value)?;
                shared.guard_reach(&ctx.actor, &id, false)?;
                if shared.rail().node(&id)?.kind == NodeKind::Workstream {
                    let removal = shared.remove_workstream(ctx.actor.clone(), &id).await?;
                    for item in &removal.removed {
                        Self::touch_rail(ctx, &item.id)?;
                    }
                    for item in &removal.kept {
                        Self::touch_rail(ctx, &item.id)?;
                    }
                    Self::touch_rail(ctx, &id)?;
                    return reply(&removal);
                }
                shared.remove(ctx.actor.clone(), &id).await?;
                Self::touch_rail(ctx, &id)?;
                reply(&())
            }
            _ => Err(RpcError::method_not_found(method)),
        }
    }
}

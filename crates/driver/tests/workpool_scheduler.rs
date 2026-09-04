//! Joined workpool scheduling, persistence, and owner-lifecycle contracts.

use std::{
	sync::{
		Arc,
		atomic::{AtomicBool, AtomicUsize, Ordering},
	},
	time::Duration,
};

use async_trait::async_trait;
use omp_agent::{JobBoard, SessionTopology, Up, jobs::undelivered};
use omp_core::{Str, sf};
use omp_driver::{
	sessions::{IrcRelayPolicy, KernelHandle, SessionId, SessionRegistry},
	subagent::{
		workpool::WorkpoolRegistry,
		workpool_scheduler::{
			SchedulerRegistry, WorkerBatch, WorkerEvent, WorkerHandle, WorkerSpawn, WorkpoolCreate,
			WorkpoolLauncher, WorkpoolPolicy, WorkpoolSchedulerError,
		},
	},
};
use omp_session::{ComponentRegistry, Session};
use parking_lot::RwLock;
use serde_json::json;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

struct Policy {
	limit: usize,
	fresh: bool,
}

impl WorkpoolPolicy for Policy {
	fn concurrency_limit(&self) -> usize {
		self.limit
	}

	fn fresh_agents(&self) -> bool {
		self.fresh
	}

	fn eval_tools_enabled(&self) -> bool {
		true
	}
}

struct Launcher {
	sessions:  Arc<SessionRegistry>,
	snapshot:  Arc<RwLock<omp_dom::Snapshot>>,
	main:      Str,
	spawned:   AtomicUsize,
	active:    Arc<AtomicUsize>,
	maximum:   Arc<AtomicUsize>,
	die_once:  Arc<AtomicBool>,
	forwarded: Arc<RwLock<Option<Arc<omp_tools::eval::EvalToolRoster>>>>,
}

#[async_trait]
impl WorkpoolLauncher for Launcher {
	async fn spawn(
		&self,
		request: WorkerSpawn,
		events: flume::Sender<WorkerEvent>,
	) -> Result<WorkerHandle, WorkpoolSchedulerError> {
		self.spawned.fetch_add(1, Ordering::Relaxed);
		*self.forwarded.write() = request.eval_tools.clone();
		let (batches, batch_rx) = flume::unbounded::<WorkerBatch>();
		let cancel = CancellationToken::new();
		let (mailbox, mailbox_rx) = flume::unbounded::<Up>();
		self.sessions.register(request.id.clone(), KernelHandle {
			id:        SessionId::new(request.id.clone()),
			name:      request.id.clone(),
			up:        mailbox,
			snapshot:  Arc::clone(&self.snapshot),
			topology:  SessionTopology::child(request.owner.clone(), self.main.clone()),
			relay:     IrcRelayPolicy::fixed(true),
			autoreply: None,
		});
		let id = request.id.clone();
		let child_cancel = cancel.clone();
		let active = Arc::clone(&self.active);
		let maximum = Arc::clone(&self.maximum);
		let die_once = Arc::clone(&self.die_once);
		tokio::spawn(async move {
			let _mailbox_rx = mailbox_rx;
			loop {
				let batch = tokio::select! {
					() = child_cancel.cancelled() => break,
					batch = batch_rx.recv_async() => match batch {
						Ok(batch) => batch,
						Err(_) => break,
					},
				};
				let width = active.fetch_add(1, Ordering::SeqCst) + 1;
				maximum.fetch_max(width, Ordering::SeqCst);
				let delay = if batch
					.items
					.iter()
					.any(|(_, text)| text.as_str() == "running")
				{
					250
				} else {
					15
				};
				tokio::time::sleep(Duration::from_millis(delay)).await;
				active.fetch_sub(1, Ordering::SeqCst);
				if batch.items.iter().any(|(_, text)| text.as_str() == "die")
					&& die_once.swap(false, Ordering::SeqCst)
				{
					let _ = events
						.send_async(WorkerEvent::Dead {
							worker: id.clone(),
							error:  Str::new_static("simulated worker death"),
						})
						.await;
					break;
				}
				let success = !batch.items.iter().any(|(_, text)| text.as_str() == "fail");
				let output = batch
					.items
					.iter()
					.map(|(item, text)| sf!("{item}: {text}"))
					.collect::<Vec<_>>()
					.join("\n");
				let _ = events
					.send_async(WorkerEvent::Settled {
						worker: id.clone(),
						batch: batch.id,
						output: Str::new(output),
						success,
						alive: true,
						context_tokens: Some(100),
						context_window: Some(1_000),
					})
					.await;
			}
		});
		Ok(WorkerHandle { id: request.id, batches, cancel })
	}
}

struct Harness {
	registry:     SchedulerRegistry,
	parent:       Arc<Mutex<Session>>,
	jobs:         Arc<JobBoard>,
	launcher:     Arc<Launcher>,
	producers:    Arc<WorkpoolRegistry>,
	_owner_inbox: flume::Receiver<Up>,
}

fn harness(limit: usize, fresh: bool, die_once: bool) -> Harness {
	let temp = tempfile::tempdir().expect("temporary directory");
	let path = temp.keep().join("owner.oms");
	let parent = Session::create(path, ComponentRegistry::standard()).expect("parent session");
	let spill = parent.blobs().clone();
	let snapshot = Arc::new(RwLock::new(parent.dom().snapshot()));
	let sessions = Arc::new(SessionRegistry::new());
	let (mailbox, owner_inbox) = flume::unbounded();
	sessions.register(sf!("owner"), KernelHandle {
		id:        SessionId::new(sf!("owner")),
		name:      sf!("owner"),
		up:        mailbox,
		snapshot:  Arc::clone(&snapshot),
		topology:  SessionTopology::main(sf!("owner")),
		relay:     IrcRelayPolicy::fixed(true),
		autoreply: None,
	});
	let authority: Arc<dyn omp_agent::SessionAuthority> = sessions.clone();
	let producers = Arc::new(WorkpoolRegistry::new(authority));
	let launcher = Arc::new(Launcher {
		sessions,
		snapshot,
		main: sf!("owner"),
		spawned: AtomicUsize::new(0),
		active: Arc::new(AtomicUsize::new(0)),
		maximum: Arc::new(AtomicUsize::new(0)),
		die_once: Arc::new(AtomicBool::new(die_once)),
		forwarded: Arc::new(RwLock::new(None)),
	});
	let parent = Arc::new(Mutex::new(parent));
	let jobs = Arc::new(JobBoard::new());
	let registry = SchedulerRegistry::new(
		sf!("owner"),
		Arc::clone(&parent),
		Arc::clone(&jobs),
		spill,
		Arc::clone(&producers),
		launcher.clone(),
		Arc::new(Policy { limit, fresh }),
		omp_tools::eval::EvalSessionControl::default(),
	);
	Harness { registry, parent, jobs, launcher, producers, _owner_inbox: owner_inbox }
}

async fn wait_pending(pool: &omp_driver::subagent::workpool_scheduler::Workpool, expected: usize) {
	for _ in 0..200 {
		if pool.peek().pending == expected {
			return;
		}
		tokio::time::sleep(Duration::from_millis(5)).await;
	}
	panic!("pool did not reach pending={expected}");
}

#[tokio::test]
async fn authenticated_eval_registrations_reach_each_child_with_exact_identity() {
	let harness = harness(1, false, false);
	harness
		.registry
		.bridge_call(json!({
			"op": "create",
			"name": "eval-pool",
			"agent": "task",
			"tools": ["score"],
			"tool_registrations": [{
				"name": "score",
				"description": "Score one candidate",
				"parameters": {
					"type": "object",
					"properties": { "candidate": { "type": "string" } },
					"required": ["candidate"],
					"additionalProperties": false
				},
				"rev": 7,
				"handler": "0123456789abcdef0123456789abcdef",
				"generation": 4
			}]
		}))
		.await
		.expect("authenticated bridge registration");
	let pool = harness.registry.get("eval-pool").expect("created pool");
	pool.push(vec![sf!("candidate")]).await.expect("queue item");
	wait_pending(&pool, 0).await;

	let forwarded = harness.launcher.forwarded.read();
	let roster = forwarded.as_ref().expect("forwarded roster");
	assert_eq!(roster.generation, 4);
	assert_eq!(roster.tools.len(), 1);
	assert_eq!(roster.tools[0].name, "score");
	assert_eq!(roster.tools[0].rev, 7);
	assert_eq!(roster.tools[0].handler, "0123456789abcdef0123456789abcdef");
	assert_eq!(roster.tools[0].parameters["required"], json!(["candidate"]));
	drop(forwarded);
	pool.close().await.expect("close pool");

	assert!(matches!(
		harness
			.registry
			.bridge_call(json!({
				"op": "create",
				"name": "forged",
				"tools": ["score"],
				"tool_registrations": []
			}))
			.await,
		Err(WorkpoolSchedulerError::EvalToolRegistrationMismatch)
	));
}

#[tokio::test]
async fn persistent_workers_batch_queue_and_aggregate_delivery_stays_atomic() {
	let harness = harness(1, false, false);
	let pool = harness
		.registry
		.create(WorkpoolCreate {
			name:    sf!("audit"),
			agent:   sf!("task"),
			context: Some(sf!("shared context")),
		})
		.expect("create pool");
	assert_eq!(
		pool
			.push(vec![sf!("one"), sf!("two"), sf!("three")])
			.await
			.expect("push")
			.len(),
		3
	);
	wait_pending(&pool, 0).await;
	let status = pool.status();
	assert_eq!(status.agents.len(), 1);
	assert_eq!(status.agents[0].turns, 2);
	assert_eq!(status.batches, 2);
	assert_eq!(status.items.completed, 3);
	assert!(undelivered(harness.parent.lock().await.dom()).is_empty());
	assert!(pool.close().await.expect("close").is_empty());
	let mut parent = harness.parent.lock().await;
	let settled = harness
		.jobs
		.wait(&mut parent, Some(&[sf!("audit")]))
		.await
		.expect("wait aggregate")
		.expect("aggregate job");
	drop(parent);
	assert_eq!(settled.status, "completed");
	let aggregate: serde_json::Value =
		serde_json::from_str(settled.output.as_deref().expect("aggregate output").get())
			.expect("aggregate JSON");
	let text = aggregate["text"].as_str().expect("aggregate text");
	let first = text.find("[audit#1]").expect("first item");
	let second = text.find("[audit#2]").expect("second item");
	let third = text.find("[audit#3]").expect("third item");
	assert!(first < second && second < third, "aggregate preserves push order");
	assert!(
		text.find("## Items").expect("item section")
			< text.find("## Batch attempts").expect("attempts")
	);
	let pending = undelivered(harness.parent.lock().await.dom());
	assert_eq!(pending.len(), 1);
	assert_eq!(pending[0].id, "audit");
	let _ = pool.peek();
	assert_eq!(undelivered(harness.parent.lock().await.dom()).len(), 1);
}

#[tokio::test]
async fn failed_batch_marks_every_correlated_item_failed() {
	let harness = harness(1, false, false);
	let pool = harness
		.registry
		.create(WorkpoolCreate { name: sf!("failure"), agent: sf!("task"), context: None })
		.expect("create pool");
	let ids = pool
		.push(vec![sf!("first"), sf!("fail"), sf!("third")])
		.await
		.expect("push");
	wait_pending(&pool, 0).await;
	assert_eq!(ids, vec![sf!("failure#1"), sf!("failure#2"), sf!("failure#3")]);
	let status = pool.status();
	assert_eq!(status.items.completed, 1);
	assert_eq!(status.items.failed, 2);
	assert_eq!(status.items.cancelled, 0);
}

#[tokio::test]
async fn dead_worker_requeues_active_work_on_a_replacement() {
	let harness = harness(1, false, true);
	let pool = harness
		.registry
		.create(WorkpoolCreate { name: sf!("recovery"), agent: sf!("task"), context: None })
		.expect("create pool");
	pool.push(vec![sf!("die")]).await.expect("push");
	wait_pending(&pool, 0).await;
	let status = pool.status();
	assert_eq!(status.items.completed, 1);
	assert_eq!(status.batches, 2);
	assert_eq!(harness.launcher.spawned.load(Ordering::Relaxed), 2);
}

#[tokio::test]
async fn fresh_policy_honors_concurrency_and_uses_one_worker_per_item() {
	let harness = harness(2, true, false);
	let pool = harness
		.registry
		.create(WorkpoolCreate { name: sf!("fresh"), agent: sf!("task"), context: None })
		.expect("create pool");
	pool
		.push(vec![sf!("a"), sf!("b"), sf!("c"), sf!("d")])
		.await
		.expect("push");
	wait_pending(&pool, 0).await;
	assert_eq!(harness.launcher.spawned.load(Ordering::Relaxed), 4);
	assert!(harness.launcher.maximum.load(Ordering::Relaxed) <= 2);
	assert_eq!(pool.status().items.completed, 4);
}

#[tokio::test]
async fn owner_release_cancels_pool_and_revokes_its_authenticated_producer() {
	let harness = harness(1, false, false);
	let pool = harness
		.registry
		.create(WorkpoolCreate { name: sf!("reset"), agent: sf!("task"), context: None })
		.expect("create pool");
	pool.push(vec![sf!("running")]).await.expect("push");
	for _ in 0..200 {
		if !pool.peek().batches.is_empty() {
			break;
		}
		tokio::time::sleep(Duration::from_millis(5)).await;
	}
	assert!(!pool.peek().batches.is_empty(), "active batch was dispatched");
	harness.registry.release_owner();
	let mut parent = harness.parent.lock().await;
	let settled = harness
		.jobs
		.wait(&mut parent, Some(&[sf!("reset")]))
		.await
		.expect("wait cancelled")
		.expect("aggregate job");
	drop(parent);
	assert_eq!(settled.status, "cancelled");
	let status = pool.status();
	assert_eq!(status.items.cancelled, 1);
	assert_eq!(pool.peek().batches[0].status, "cancelled");
	assert!(harness.producers.get("owner", "reset").is_none());
}

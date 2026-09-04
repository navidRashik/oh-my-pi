//! Goal-mode Director.

use std::fmt::Write as _;

use omp_core::Str;
use omp_dom::{Dom, Node};

use crate::director::{
	BindValue, Director, DirectorCx, DirectorEffect, Slot, StateUpdate, TurnView, Verdict,
	director_status, find_director, state_bool, state_int, state_str, turn_call_inputs, turn_tokens,
};

const CLAIMS: &[Slot] = &[Slot::Mode, Slot::Loop];
const ACTIVE: &str = "active";

/// Whether the selected branch has an active Goal eligible for idle
/// continuation.
#[must_use]
pub fn continuation_is_active(dom: &Dom) -> bool {
	find_director(dom, "goal").is_some_and(|(_, node)| director_status(node) == Some(ACTIVE))
}

/// Builds the hidden prompt for the next idle-boundary continuation.
///
/// An active Goal yields after a prose-only model response. The interactive
/// controller may submit this prompt as a new turn after pi's 800 ms idle
/// window; paused and queued engagements deliberately produce no prompt.
#[must_use]
pub fn continuation_prompt(dom: &Dom) -> Option<Str> {
	let (_, node) = find_director(dom, "goal")?;
	if director_status(node) != Some(ACTIVE) {
		return None;
	}
	let objective = state_str(node, "objective")?;
	let tokens_used = state_int(node, "tokens_used")
		.and_then(|value| u64::try_from(value).ok())
		.unwrap_or(0);
	let token_budget = state_int(node, "token_budget").and_then(|value| u64::try_from(value).ok());
	let (budget, remaining) = token_budget.map_or_else(
		|| (Str::new_static("none"), Str::new_static("unbounded")),
		|budget| {
			(Str::new(budget.to_string()), Str::new(budget.saturating_sub(tokens_used).to_string()))
		},
	);
	let mut prompt = String::with_capacity(objective.len().saturating_add(640));
	prompt.push_str("Continue active goal.\n\n<objective>\n");
	push_xml_text(&mut prompt, objective.as_str());
	write!(
		&mut prompt,
		"\n</objective>\n\nBudget:\n- Tokens used: {tokens_used}\n- Token budget: {budget}\n- \
		 Tokens remaining: {remaining}\n\nAutonomous continuation; objective persists across turns. \
		 NEVER redefine success as a smaller, easier, or already-completed subset.\n\nBefore \
		 `goal({{op:\"complete\"}})`, audit the current repo state and verify every objective \
		 deliverable with direct current-state evidence. Uncertainty means the goal is unfinished. \
		 Budget exhaustion is not completion. If unfinished, keep working without narrating \
		 continuation."
	)
	.expect("formatting a String is infallible");
	Some(Str::new(prompt))
}

fn push_xml_text(out: &mut String, text: &str) {
	for character in text.chars() {
		match character {
			'&' => out.push_str("&amp;"),
			'<' => out.push_str("&lt;"),
			'>' => out.push_str("&gt;"),
			_ => out.push(character),
		}
	}
}

/// Keeps the loop occupied until a goal completes, drops, or exhausts its token
/// budget.
pub struct Goal {
	objective:    Str,
	token_budget: Option<u64>,
	tokens_used:  u64,
	done:         bool,
	dropped:      bool,
}

impl Goal {
	/// Creates a goal engagement.
	#[must_use]
	pub fn new(objective: impl Into<Str>, token_budget: Option<u64>) -> Self {
		Self {
			objective: objective.into(),
			token_budget,
			tokens_used: 0,
			done: false,
			dropped: false,
		}
	}

	/// Reconstructs goal state from its DOM element.
	#[must_use]
	pub fn from_node(node: &Node) -> Self {
		Self {
			objective:    state_str(node, "objective").unwrap_or_default(),
			token_budget: state_int(node, "token_budget").and_then(|value| u64::try_from(value).ok()),
			tokens_used:  state_int(node, "tokens_used")
				.and_then(|value| u64::try_from(value).ok())
				.unwrap_or(0),
			done:         state_bool(node, "done").unwrap_or(false),
			dropped:      state_bool(node, "dropped").unwrap_or(false),
		}
	}
}

impl Director for Goal {
	fn id(&self) -> &'static str {
		"goal"
	}

	fn claims(&self) -> &'static [Slot] {
		CLAIMS
	}

	fn state(&self) -> Vec<(Str, BindValue)> {
		vec![
			(Str::new_static("objective"), BindValue::Str(self.objective.clone())),
			(
				Str::new_static("token_budget"),
				BindValue::Int(
					self
						.token_budget
						.and_then(|value| i64::try_from(value).ok())
						.unwrap_or(-1),
				),
			),
			(
				Str::new_static("tokens_used"),
				BindValue::Int(i64::try_from(self.tokens_used).unwrap_or(i64::MAX)),
			),
			(Str::new_static("done"), BindValue::Bool(self.done)),
			(Str::new_static("dropped"), BindValue::Bool(self.dropped)),
			(Str::new_static("tool"), BindValue::Str(Str::new_static("goal"))),
		]
	}

	fn observe_turn(&self, dom: &Dom, _cx: &DirectorCx<'_>, turn: &TurnView) -> Vec<StateUpdate> {
		let mut updates = vec![StateUpdate::new(
			"tokens_used",
			BindValue::Int(
				i64::try_from(self.tokens_used.saturating_add(turn_tokens(dom, turn.turn)))
					.unwrap_or(i64::MAX),
			),
		)];
		for input in turn_call_inputs(dom, turn.turn, "goal") {
			let op = serde_json::from_str::<serde_json::Value>(input)
				.ok()
				.and_then(|value| {
					value
						.get("op")
						.and_then(|op| op.as_str())
						.map(str::to_owned)
				});
			match op.as_deref() {
				Some("complete") => updates.push(StateUpdate::new("done", BindValue::Bool(true))),
				Some("drop") => updates.push(StateUpdate::new("dropped", BindValue::Bool(true))),
				_ => {},
			}
		}
		updates
	}

	fn evaluate(&self, _dom: &Dom, _cx: &DirectorCx<'_>, turn: &TurnView) -> DirectorEffect {
		if self.done || self.dropped {
			return DirectorEffect::new(Verdict::Done);
		}
		if self
			.token_budget
			.is_some_and(|budget| self.tokens_used >= budget)
		{
			return DirectorEffect::new(Verdict::Done)
				.with_aside("Goal token budget exhausted; returning control to the user.");
		}
		if !turn.had_tool_calls {
			return DirectorEffect::new(Verdict::Yield);
		}
		DirectorEffect::new(Verdict::Continue { reminder: None })
	}
}

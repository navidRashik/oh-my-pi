//! Mechanical projection of pi's complete Memory settings tab.

use super::*;

pub(super) const ENTRIES: &[UiSpec] = &[
	ui!(
		"memory.backend",
		"ai_memory_backend",
		Memory,
		"General",
		"Memory Backend",
		"Off, local summary pipeline, Mnemopi SQLite, Hindsight remote memory, or Sharpshooter",
		UiWidget::Submenu(&[
			UiOption::new("off", "Off", "No memory subsystem runs"),
			UiOption::new(
				"local",
				"Local",
				"Local rollout summarisation pipeline (memory_summary.md)"
			),
			UiOption::new("hindsight", "Hindsight", "Vectorize Hindsight remote memory service"),
			UiOption::new(
				"mnemopi",
				"Mnemopi",
				"Local SQLite recall/retain backend with optional embeddings"
			),
			UiOption::new(
				"sharpshooter",
				"Sharpshooter",
				"Friction-gated project decision files (architecture/product/style), consolidated in \
				 the background"
			)
		]),
		None,
		Identity
	),
	ui!(
		"providers.memoryModel",
		"ai_memory_selector",
		Memory,
		"General",
		"Memory Model",
		"Mnemopi LLM for fact extraction + consolidation: online (the TINY role from /models, else \
		 smol/remote) by default, or a local on-device model",
		UiWidget::Submenu(&[
			UiOption::new(
				"online",
				"Online (TINY role, else @smol)",
				"Use the online model: the TINY role from /models when set, otherwise @smol. No local \
				 model download or on-device inference."
			),
			UiOption::new(
				"qwen3-1.7b",
				"Qwen3 1.7B",
				"MLX only (providers.tinyModelDevice=mlx): onnxruntime-node cannot run this ONNX \
				 export's RotaryEmbedding cache updates."
			),
			UiOption::new(
				"llama3.2:3b",
				"Llama 3.2 3B",
				"Larger Llama 3.2 option for local memory/classifier tasks; higher quality potential \
				 at higher disk/RAM/latency cost."
			),
			UiOption::new(
				"gemma-3-1b",
				"Gemma 3 1B",
				"Best consolidation/dedup; lighter footprint, but leaks small talk during extraction."
			),
			UiOption::new(
				"qwen2.5-1.5b",
				"Qwen2.5 1.5B",
				"Best extraction granularity (atomic facts); weaker consolidation."
			),
			UiOption::new(
				"lfm2-1.2b",
				"LFM2 1.2B",
				"Fastest load; solid all-rounder, slightly noisier extraction labels."
			)
		]),
		Some(UiCondition::MnemopiActive),
		OnlineTinyModel
	),
	ui!(
		"autolearn.enabled",
		"ai_autolearn_enabled",
		Memory,
		"Auto-Learn",
		"Auto-Learn (experimental)",
		"After the agent stops, nudge it to capture lessons to memory and create/enhance isolated \
		 managed skills",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"autolearn.autoContinue",
		"ai_autolearn_auto_continue",
		Memory,
		"Auto-Learn",
		"Auto-run capture at stop",
		"When on, auto-run one private capture turn at stop (uses extra tokens). When off, only \
		 standing auto-learn guidance remains.",
		UiWidget::Boolean,
		Some(UiCondition::AutolearnActive),
		Identity
	),
	ui!(
		"mnemopi.dbPath",
		"ai_mnemopi_db_path",
		Memory,
		"Mnemopi",
		"Mnemopi DB Path",
		"Optional SQLite DB path. Defaults to the agent memories directory.",
		UiWidget::Text { secret: false },
		Some(UiCondition::MnemopiActive),
		Identity
	),
	ui!(
		"mnemopi.bank",
		"ai_mnemopi_bank",
		Memory,
		"Mnemopi",
		"Mnemopi Bank",
		"Optional shared bank base name. Per-project modes derive project-local banks from it.",
		UiWidget::Text { secret: false },
		Some(UiCondition::MnemopiActive),
		Identity
	),
	ui!(
		"mnemopi.scoping",
		"ai_mnemopi_scoping",
		Memory,
		"Mnemopi",
		"Mnemopi Scoping",
		"global = one shared bank; per-project = isolated bank per cwd; per-project-tagged = \
		 project-local writes plus global recall visibility",
		UiWidget::Submenu(&[
			UiOption::new("global", "Global", "One shared Mnemopi bank for every project"),
			UiOption::new("per-project", "Per project", "Project-local Mnemopi bank per cwd basename"),
			UiOption::new(
				"per-project-tagged",
				"Per project (tagged)",
				"Write to a project-local bank but merge project + shared recall results"
			)
		]),
		Some(UiCondition::MnemopiActive),
		Identity
	),
	ui!(
		"mnemopi.embeddingVariant",
		"ai_mnemopi_embedding_variant",
		Memory,
		"Mnemopi",
		"Embedding variant",
		"Local embedding model family. en = stronger English model; multilingual = cross-language \
		 model. Changing this rebuilds existing memory embeddings on next start.",
		UiWidget::Submenu(&[
			UiOption::new(
				"en",
				"English (bge-base-en-v1.5)",
				"BAAI/bge-base-en-v1.5 (768d), English-only"
			),
			UiOption::new(
				"multilingual",
				"Multilingual (multilingual-e5-large)",
				"intfloat/multilingual-e5-large (1024d), cross-language recall"
			)
		]),
		Some(UiCondition::MnemopiActive),
		Identity
	),
	ui!(
		"mnemopi.autoRecall",
		"ai_mnemopi_auto_recall",
		Memory,
		"Mnemopi",
		"Mnemopi Auto Recall",
		"Recall local memories into the first turn of each session",
		UiWidget::Boolean,
		Some(UiCondition::MnemopiActive),
		Identity
	),
	ui!(
		"mnemopi.autoRetain",
		"ai_mnemopi_auto_retain",
		Memory,
		"Mnemopi",
		"Mnemopi Auto Retain",
		"Retain completed conversation turns into local Mnemopi memory",
		UiWidget::Boolean,
		Some(UiCondition::MnemopiActive),
		Identity
	),
	ui!(
		"mnemopi.polyphonicRecall",
		"ai_mnemopi_polyphonic_recall",
		Memory,
		"Mnemopi",
		"Mnemopi Polyphonic Recall",
		"Enable 4-voice recall (vector, graph, fact, temporal) fused with reciprocal rank fusion",
		UiWidget::Boolean,
		Some(UiCondition::MnemopiActive),
		Identity
	),
	ui!(
		"mnemopi.enhancedRecall",
		"ai_mnemopi_enhanced_recall",
		Memory,
		"Mnemopi",
		"Mnemopi Enhanced Recall",
		"Enable the tiered query result cache for repeated and similar recall queries",
		UiWidget::Boolean,
		Some(UiCondition::MnemopiActive),
		Identity
	),
	ui!(
		"mnemopi.proactiveLinking",
		"ai_mnemopi_proactive_linking",
		Memory,
		"Mnemopi",
		"Mnemopi Proactive Linking",
		"Ingest new memories into the episodic graph as they are stored, linking them to related \
		 entities and memories",
		UiWidget::Boolean,
		Some(UiCondition::MnemopiActive),
		Identity
	),
	ui!(
		"mnemopi.noEmbeddings",
		"ai_mnemopi_no_embeddings",
		Memory,
		"Mnemopi",
		"Mnemopi Disable Embeddings",
		"Force deterministic FTS-only recall instead of vector embeddings",
		UiWidget::Boolean,
		Some(UiCondition::MnemopiActive),
		Identity
	),
	ui!(
		"mnemopi.embeddingModel",
		"ai_mnemopi_embedding_model",
		Memory,
		"Mnemopi",
		"Mnemopi Embedding Model",
		"Advanced: explicit embedding model id that overrides the variant. Leave empty to use \
		 mnemopi.embeddingVariant.",
		UiWidget::Text { secret: false },
		Some(UiCondition::MnemopiActive),
		Identity
	),
	ui!(
		"mnemopi.embeddingApiUrl",
		"ai_mnemopi_embedding_api_url",
		Memory,
		"Mnemopi",
		"Mnemopi Embedding API URL",
		"Optional OpenAI-compatible embedding endpoint passed to Mnemopi",
		UiWidget::Text { secret: false },
		Some(UiCondition::MnemopiActive),
		Identity
	),
	ui!(
		"mnemopi.embeddingApiKey",
		"ai_mnemopi_embedding_api_key",
		Memory,
		"Mnemopi",
		"Mnemopi Embedding API Key",
		"Optional embedding API key passed to Mnemopi",
		UiWidget::Text { secret: true },
		Some(UiCondition::MnemopiActive),
		Identity
	),
	ui!(
		"mnemopi.llmMode",
		"ai_mnemopi_llm_mode",
		Memory,
		"Mnemopi",
		"Mnemopi LLM Mode",
		"Use no LLM, the online tiny model (the TINY role from /models, else @smol), or a remote \
		 OpenAI-compatible endpoint",
		UiWidget::Submenu(&[
			UiOption::new("none", "None", "Disable Mnemopi LLM-backed extraction"),
			UiOption::new(
				"smol",
				"Online (tiny)",
				"Use the online tiny model (the TINY role from /models, else @smol)"
			),
			UiOption::new("remote", "Remote", "Use the Mnemopi remote LLM settings below")
		]),
		Some(UiCondition::MnemopiActive),
		Identity
	),
	ui!(
		"mnemopi.llmBaseUrl",
		"ai_mnemopi_llm_base_url",
		Memory,
		"Mnemopi",
		"Mnemopi LLM Base URL",
		"Optional OpenAI-compatible LLM endpoint for Mnemopi remote mode",
		UiWidget::Text { secret: false },
		Some(UiCondition::MnemopiActive),
		Identity
	),
	ui!(
		"mnemopi.llmApiKey",
		"ai_mnemopi_llm_api_key",
		Memory,
		"Mnemopi",
		"Mnemopi LLM API Key",
		"Optional LLM API key for Mnemopi remote mode",
		UiWidget::Text { secret: true },
		Some(UiCondition::MnemopiActive),
		Identity
	),
	ui!(
		"mnemopi.llmModel",
		"ai_mnemopi_llm_model",
		Memory,
		"Mnemopi",
		"Mnemopi LLM Model",
		"Optional LLM model name for Mnemopi remote mode",
		UiWidget::Text { secret: false },
		Some(UiCondition::MnemopiActive),
		Identity
	),
	ui!(
		"hindsight.apiUrl",
		"ai_hindsight_api_url",
		Memory,
		"Hindsight",
		"Hindsight API URL",
		"Hindsight server URL (Cloud or self-hosted)",
		UiWidget::Text { secret: false },
		Some(UiCondition::HindsightActive),
		Identity
	),
	ui!(
		"hindsight.apiToken",
		"ai_hindsight_api_token",
		Memory,
		"Hindsight",
		"Hindsight API Token",
		"Bearer token for authenticated Hindsight servers",
		UiWidget::Text { secret: true },
		Some(UiCondition::HindsightActive),
		Identity
	),
	ui!(
		"hindsight.bankId",
		"ai_hindsight_bank_id",
		Memory,
		"Hindsight",
		"Hindsight Bank ID",
		"Memory bank identifier (default: project name)",
		UiWidget::Text { secret: false },
		Some(UiCondition::HindsightActive),
		Identity
	),
	ui!(
		"hindsight.scoping",
		"ai_hindsight_scoping",
		Memory,
		"Hindsight",
		"Hindsight Scoping",
		"global = one shared bank; per-project = isolated bank per cwd; per-project-tagged = shared \
		 bank with project tags so global + project memories merge on recall",
		UiWidget::Submenu(&[
			UiOption::new(
				"global",
				"Global",
				"One shared bank — every project sees the same memories"
			),
			UiOption::new(
				"per-project",
				"Per project",
				"Isolated bank per cwd basename — projects cannot see each other's memories"
			),
			UiOption::new(
				"per-project-tagged",
				"Per project (tagged)",
				"Shared bank, retains tagged with project:<cwd>. Recall surfaces project + untagged \
				 global memories together"
			)
		]),
		Some(UiCondition::HindsightActive),
		Identity
	),
	ui!(
		"hindsight.autoRecall",
		"ai_hindsight_auto_recall",
		Memory,
		"Hindsight",
		"Hindsight Auto Recall",
		"Recall memories on the first turn of each session",
		UiWidget::Boolean,
		Some(UiCondition::HindsightActive),
		Identity
	),
	ui!(
		"hindsight.autoRetain",
		"ai_hindsight_auto_retain",
		Memory,
		"Hindsight",
		"Hindsight Auto Retain",
		"Retain transcript every N turns and at session boundaries",
		UiWidget::Boolean,
		Some(UiCondition::HindsightActive),
		Identity
	),
	ui!(
		"hindsight.retainMode",
		"ai_hindsight_retain_mode",
		Memory,
		"Hindsight",
		"Hindsight Retain Mode",
		"full-session = upsert one document per session, last-turn = chunked",
		UiWidget::Submenu(&[
			UiOption::new(
				"full-session",
				"Full session",
				"Upsert one document per session (recommended)"
			),
			UiOption::new("last-turn", "Last turn", "Chunked retention sliced by turn boundaries")
		]),
		Some(UiCondition::HindsightActive),
		Identity
	),
	ui!(
		"hindsight.mentalModelsEnabled",
		"ai_hindsight_mental_models_enabled",
		Memory,
		"Hindsight",
		"Hindsight Mental Models",
		"Read curated reflect summaries (mental models) into developer instructions at boot. Loads \
		 existing models on the bank — does not write. Pair with hindsight.mentalModelAutoSeed to \
		 also auto-create the built-in seed set.",
		UiWidget::Boolean,
		Some(UiCondition::HindsightActive),
		Identity
	),
	ui!(
		"hindsight.mentalModelAutoSeed",
		"ai_hindsight_mental_model_auto_seed",
		Memory,
		"Hindsight",
		"Hindsight Mental Model Auto-Seed",
		"At session start, create any built-in mental models (project-conventions, \
		 project-decisions, user-preferences) that do not yet exist on the bank.",
		UiWidget::Boolean,
		Some(UiCondition::HindsightActive),
		Identity
	),
	ui!(
		"sharpshooter.model",
		"ai_sharpshooter_model",
		Memory,
		"Sharpshooter",
		"Sharpshooter Model",
		"Model selector for extraction/consolidation, empty = smol role",
		UiWidget::Text { secret: false },
		None,
		Identity
	),
];

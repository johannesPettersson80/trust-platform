//! The five composition patterns (specification 43, §6 and AT-02).
//!
//! Port of `agent/patterns.ts`: the design guidance of each pattern, its visual-review status
//! (`pending` until its reference images are reviewed; availability is not approval) and the text
//! filter of the library. The IDs are the exact values a composition request accepts.

use serde::Serialize;

use super::layout::CompositionRole;

/// A composition pattern's stable ID (`CompositionPattern["id"]` in `patterns.ts`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum PatternId {
    /// A process with a dominant process area and supporting measurements.
    ProcessOverview,
    /// One equipment unit with its measurements and its controls.
    EquipmentDetail,
    /// Repeated equipment units with equal visual priority.
    MultiUnit,
    /// Trends and alarms for an investigation.
    Investigation,
    /// Grouped settings and commissioning values.
    Settings,
}

impl PatternId {
    /// Every pattern, in the order the library lists them.
    pub(super) const ALL: [Self; 5] = [
        Self::ProcessOverview,
        Self::EquipmentDetail,
        Self::MultiUnit,
        Self::Investigation,
        Self::Settings,
    ];

    /// The pattern named by `text` (its kebab-case ID), if it is one of the five.
    #[must_use]
    pub(super) fn parse(text: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|pattern| pattern.as_str() == text)
    }

    /// The pattern's ID as the tool contract writes it.
    #[must_use]
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::ProcessOverview => "process-overview",
            Self::EquipmentDetail => "equipment-detail",
            Self::MultiUnit => "multi-unit",
            Self::Investigation => "investigation",
            Self::Settings => "settings",
        }
    }
}

/// Where a pattern puts one kind of content, and why.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(super) struct Hierarchy {
    /// The region role.
    pub(super) region: CompositionRole,
    /// What the region is for.
    pub(super) purpose: &'static str,
}

/// The spacing the patterns share, in design pixels (specification 39's 8-pixel grid).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(super) struct Spacing {
    /// The grid unit.
    pub(super) grid: u32,
    /// The page margin.
    pub(super) margin: u32,
    /// The default gap between regions.
    pub(super) gap: u32,
}

/// One composition pattern: its guidance as the library reports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CompositionPattern {
    /// The stable ID.
    pub(super) id: PatternId,
    /// The version of the guidance.
    pub(super) version: u32,
    /// The title shown to the operator.
    pub(super) title: &'static str,
    /// The operator task the pattern serves.
    pub(super) task: &'static str,
    /// What the project must supply before the pattern can be filled.
    pub(super) required_inputs: &'static [&'static str],
    /// The regions the pattern uses, in hierarchy order.
    pub(super) hierarchy: &'static [Hierarchy],
    /// The spacing the pattern uses.
    pub(super) spacing: Spacing,
    /// Density guidance.
    pub(super) density: &'static str,
    /// The limits that always apply to the pattern.
    pub(super) limits: &'static [&'static str],
    /// Visual-review status; always `pending` until reference images are reviewed.
    pub(super) visual_review: &'static str,
}

/// The spacing of every pattern (`spacing` in `patterns.ts`).
const SPACING: Spacing = Spacing {
    grid: 8,
    margin: 32,
    gap: 24,
};

/// The limits that apply to every pattern (`limits` in `patterns.ts`).
const LIMITS: &[&str] = &[
    "Explicit sizes and pinned equipment are preserved.",
    "Overfull regions require more space or another page; readable sizes are never reduced.",
    "Synthetic previews do not prove runtime operation.",
];

const PROCESS_OVERVIEW_HIERARCHY: &[Hierarchy] = &[
    Hierarchy {
        region: CompositionRole::Process,
        purpose: "Dominant process area; roughly two thirds of the upper width.",
    },
    Hierarchy {
        region: CompositionRole::Measurements,
        purpose: "Supporting readings in a consistent side column.",
    },
    Hierarchy {
        region: CompositionRole::Trends,
        purpose: "Optional lower history area.",
    },
    Hierarchy {
        region: CompositionRole::Alarms,
        purpose: "Optional lower alarm summary.",
    },
];

const EQUIPMENT_DETAIL_HIERARCHY: &[Hierarchy] = &[
    Hierarchy {
        region: CompositionRole::Process,
        purpose: "One primary equipment symbol or placed faceplate.",
    },
    Hierarchy {
        region: CompositionRole::Measurements,
        purpose: "Current measurements and relevant operating limits.",
    },
    Hierarchy {
        region: CompositionRole::Controls,
        purpose: "Clearly separated control or faceplate entry points.",
    },
    Hierarchy {
        region: CompositionRole::Trends,
        purpose: "Optional diagnostic history.",
    },
];

const MULTI_UNIT_HIERARCHY: &[Hierarchy] = &[
    Hierarchy {
        region: CompositionRole::Process,
        purpose: "Equal card grid in stable equipment order.",
    },
    Hierarchy {
        region: CompositionRole::Trends,
        purpose: "Optional shared history beneath the cards.",
    },
    Hierarchy {
        region: CompositionRole::Alarms,
        purpose: "Optional shared alarm area.",
    },
];

const INVESTIGATION_HIERARCHY: &[Hierarchy] = &[
    Hierarchy {
        region: CompositionRole::Trends,
        purpose: "Primary trend area with a readable time axis.",
    },
    Hierarchy {
        region: CompositionRole::Alarms,
        purpose: "Adjacent alarm investigation area with equipment navigation.",
    },
];

const SETTINGS_HIERARCHY: &[Hierarchy] = &[
    Hierarchy {
        region: CompositionRole::Controls,
        purpose: "Clearly labelled editable values with units and limits.",
    },
    Hierarchy {
        region: CompositionRole::Measurements,
        purpose: "Supporting readback for the same equipment.",
    },
];

/// Design guidance for the five supported composition IDs (`COMPOSITION_PATTERNS`). Visual
/// approval is tracked separately; a pattern ID alone never makes a result operator-ready.
static PATTERNS: [CompositionPattern; 5] = [
    CompositionPattern {
        id: PatternId::ProcessOverview,
        version: 1,
        title: "Process overview",
        task: "Understand process flow, equipment state and the measurements that need attention.",
        required_inputs: &[
            "equipment identities and readable labels",
            "confirmed port connections, including any branches",
            "measurement bindings and engineering units",
        ],
        hierarchy: PROCESS_OVERVIEW_HIERARCHY,
        spacing: SPACING,
        density: "Enlarge sparse process equipment up to 2.5 times its catalogue size. Start with 1–6 primary objects; use explicit regions for branches and instruments.",
        limits: LIMITS,
        visual_review: "pending",
    },
    CompositionPattern {
        id: PatternId::EquipmentDetail,
        version: 1,
        title: "Equipment detail",
        task: "Diagnose one equipment unit, see its measurements and reach its existing faceplate controls.",
        required_inputs: &[
            "equipment instance",
            "typed measurement and state bindings",
            "existing faceplate or navigation target",
        ],
        hierarchy: EQUIPMENT_DETAIL_HIERARCHY,
        spacing: SPACING,
        density: "One primary unit. Keep controls grouped and readable; place secondary history below the primary state.",
        limits: LIMITS,
        visual_review: "pending",
    },
    CompositionPattern {
        id: PatternId::MultiUnit,
        version: 1,
        title: "Multi-unit overview",
        task: "Compare repeated equipment units with equal visual priority and distinguish mixed states.",
        required_inputs: &[
            "unit instances",
            "matching placed faceplate definitions and equal explicit card sizes",
            "per-unit state and measurement bindings",
        ],
        hierarchy: MULTI_UNIT_HIERARCHY,
        spacing: SPACING,
        density: "Use matching card sizes and gaps. Split the page when the full card grid cannot fit at its declared readable size.",
        limits: LIMITS,
        visual_review: "pending",
    },
    CompositionPattern {
        id: PatternId::Investigation,
        version: 1,
        title: "Trends and alarms",
        task: "Investigate a disturbance by comparing trends and alarms and navigating to the affected equipment.",
        required_inputs: &[
            "trend pens with units, ranges and time span",
            "alarm scope",
            "equipment navigation targets",
        ],
        hierarchy: INVESTIGATION_HIERARCHY,
        spacing: SPACING,
        density: "Start with one trend and one alarm list. Explicit regions can give trends more width; preserve readable table rows and axes.",
        limits: LIMITS,
        visual_review: "pending",
    },
    CompositionPattern {
        id: PatternId::Settings,
        version: 1,
        title: "Settings and commissioning",
        task: "Review and change grouped commissioning settings with clear units, limits and write authority.",
        required_inputs: &[
            "writable paths and directions",
            "engineering units and permitted ranges",
            "operator labels and grouping",
        ],
        hierarchy: SETTINGS_HIERARCHY,
        spacing: SPACING,
        density: "Use a readable vertical settings group. For larger forms, split by operator task instead of squeezing more controls into one page.",
        limits: LIMITS,
        visual_review: "pending",
    },
];

/// The patterns whose ID, title or task contains `query`, ignoring case (`compositionPatterns`).
/// An empty query returns every pattern.
#[must_use]
pub(super) fn composition_patterns(query: &str) -> Vec<CompositionPattern> {
    let term = query.to_lowercase();
    PATTERNS
        .iter()
        .filter(|pattern| {
            format!("{} {} {}", pattern.id.as_str(), pattern.title, pattern.task)
                .to_lowercase()
                .contains(&term)
        })
        .cloned()
        .collect()
}

/// The pattern with this ID (`COMPOSITION_PATTERNS.find` in `intent.ts` and `patternReference`).
#[must_use]
pub(super) fn pattern(id: PatternId) -> &'static CompositionPattern {
    PATTERNS
        .iter()
        .find(|pattern| pattern.id == id)
        .unwrap_or(&PATTERNS[0])
}

pub fn supported_profile() -> PlcopenProfile {
    PlcopenProfile {
        namespace: PLCOPEN_NAMESPACE,
        profile: PROFILE_NAME,
        version: "TC6 XML v2.0 (ST-complete subset)",
        strict_subset: vec![
            "project/fileHeader/contentHeader",
            "types/pous/pou[pouType=program|function|functionBlock]",
            "pou/interface/{local|temp|input|output|inOut|external|global|access}Vars (import)",
            "types/dataTypes/dataType[baseType subset: elementary|derived|array|struct|enum|subrange] (import/export)",
            "instances/configurations/resources/tasks/program instances",
            "CODESYS addData/globalVars (import/export)",
            "CODESYS addData/method for FUNCTION_BLOCK members (import/export)",
            "CODESYS addData/projectstructure folder mapping (import/export)",
            "pou/body/ST plain-text bodies (multi-worksheet import order preserved)",
            "addData/data[name=trust.sourceMap|trust.vendorExtensions|trust.exportAdapter]",
        ],
        unsupported_nodes: vec![
            "graphical bodies (FBD/LD/SFC)",
            "vendor-specific nodes (preserved via hooks, not interpreted)",
            "dataTypes outside supported baseType subset",
        ],
        compatibility_matrix: vec![
            PlcopenCompatibilityMatrixEntry {
                capability: "POU import/export: PROGRAM/FUNCTION/FUNCTION_BLOCK with ST body",
                status: "supported",
                notes: "Aliases such as PRG/FC/FB are normalized on import. Import also reconstructs ordered TC6 body worksheets and standard interface sections, including globalVars/accessVars.",
            },
            PlcopenCompatibilityMatrixEntry {
                capability: "Source mapping metadata",
                status: "supported",
                notes: "Embedded addData trust.sourceMap + deterministic source-map sidecar JSON.",
            },
            PlcopenCompatibilityMatrixEntry {
                capability: "Vendor extension node preservation",
                status: "partial",
                notes: "Unknown addData/vendor fragments are preserved and re-injectable, but not semantically interpreted.",
            },
            PlcopenCompatibilityMatrixEntry {
                capability: "Vendor ecosystem migration heuristics",
                status: "partial",
                notes: "Detected ecosystems are advisory diagnostics for migration workflows, not semantic guarantees.",
            },
            PlcopenCompatibilityMatrixEntry {
                capability: "PLCopen dataTypes import (elementary/derived/array/struct/enum/subrange subset)",
                status: "supported",
                notes: "Supported dataType baseType nodes are imported into generated ST TYPE declarations under src/.",
            },
            PlcopenCompatibilityMatrixEntry {
                capability: "PLCopen dataTypes export (elementary/derived/array/struct/enum/subrange subset)",
                status: "partial",
                notes: "Export emits supported TYPE declarations into types/dataTypes. Unsupported ST forms are skipped with warnings.",
            },
            PlcopenCompatibilityMatrixEntry {
                capability: "Project model import/export (instances/configurations/resources/tasks/program instances)",
                status: "supported",
                notes: "ST configuration/resource/task/program-instance model is imported/exported with deterministic naming and diagnostics.",
            },
            PlcopenCompatibilityMatrixEntry {
                capability: "CODESYS global variable lists (addData/globalVars)",
                status: "supported",
                notes: "Import prefers interface-as-plaintext for VAR_GLOBAL fidelity and falls back to variable node synthesis; export emits deterministic CODESYS globalVars metadata.",
            },
            PlcopenCompatibilityMatrixEntry {
                capability: "CODESYS method members (addData/method on FUNCTION_BLOCK POUs)",
                status: "supported",
                notes: "Import reconstructs FUNCTION_BLOCK methods from vendor metadata and export emits deterministic Method objects/object IDs on the owning POU and project structure. Other vendor OOP objects such as properties remain out of scope.",
            },
            PlcopenCompatibilityMatrixEntry {
                capability: "CODESYS project structure folder mapping (addData/projectstructure)",
                status: "partial",
                notes: "Import/export mirrors deterministic source-folder hierarchies for POUs/GVLs; unsupported library/device-tree object semantics remain metadata only.",
            },
            PlcopenCompatibilityMatrixEntry {
                capability: "Vendor library compatibility shims (selected timer/edge aliases)",
                status: "partial",
                notes: "Import can normalize selected Siemens/Rockwell/Schneider/Mitsubishi aliases to IEC FB names and reports each shim application.",
            },
            PlcopenCompatibilityMatrixEntry {
                capability: "Export adapters v1 (Allen-Bradley/Siemens/Schneider)",
                status: "partial",
                notes: "Export emits target-specific adapter diagnostics/manual-step reports, but native vendor project packages remain out of scope.",
            },
            PlcopenCompatibilityMatrixEntry {
                capability: "Graphical bodies (FBD/LD/SFC) and advanced runtime deployment resources",
                status: "unsupported",
                notes: "ST-complete subset remains ST-only and does not import graphical networks or advanced deployment metadata semantics.",
            },
            PlcopenCompatibilityMatrixEntry {
                capability: "Vendor AOIs, advanced library semantics, and platform-specific pragmas",
                status: "unsupported",
                notes: "Shim catalog is intentionally narrow; unsupported content is reported in migration diagnostics and known-gaps docs.",
            },
        ],
        source_mapping: "Export writes deterministic source-map sidecar JSON and embeds trust.sourceMap in addData.",
        vendor_extension_hook:
            "Import preserves unknown addData/vendor nodes to plcopen.vendor-extensions.imported.xml; export re-injects plcopen.vendor-extensions.xml.",
        round_trip_limits: vec![
            "Round-trip guarantees preserve ST POU signatures (name/type/body intent) for ST-complete supported inputs.",
            "Round-trip guarantees preserve supported ST dataType signatures (name + supported baseType graph).",
            "Round-trip guarantees preserve supported configuration/resource/task/program-instance wiring intent.",
            "Round-trip preserves supported CODESYS globalVars declarations (plaintext-first import strategy).",
            "Round-trip preserves supported CODESYS FUNCTION_BLOCK method metadata and deterministic method object placement.",
            "Round-trip preserves deterministic folder placement intent for supported CODESYS projectstructure object trees.",
            "Round-trip does not preserve vendor formatting/layout, graphical networks, or runtime deployment metadata.",
            "Round-trip can rename output source files to sanitized unique names inside src/.",
            "Round-trip may normalize selected vendor library symbols to IEC equivalents when shim rules apply during import.",
            "Round-trip preserves unknown vendor addData as opaque fragments, not executable semantics.",
        ],
        known_gaps: vec![
            "No import/export for SFC/LD/FBD bodies.",
            "Vendor library shim coverage is limited to the published baseline alias catalog.",
            "No semantic translation for vendor-specific AOI/FB internal behavior beyond simple symbol remapping.",
            "Vendor-specific OOP objects beyond CODESYS methods (for example properties) are not yet semantically imported/exported.",
            "No guaranteed equivalence for vendor pragmas, safety metadata, or online deployment tags.",
            "Export adapters do not generate native vendor project archives (.L5X/.apxx/.project).",
        ],
    }
}

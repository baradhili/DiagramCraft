//! The built-in stencil catalog (GitHub #87; owner decision
//! 2026-10-25): synthetic `.vssx` stencils derived from the shipped
//! Visio 2016 stencils, embedded in the binary and opened from the
//! More Shapes menu like any file stencil (#83 path).
//!
//! This module is the catalog's single source of truth — slug,
//! category, display title, and embedded bytes. The asset files live
//! in `crates/dc-ui/assets/stencils/` with their manifest, digest
//! record, and provenance README; `extract-stencils.rs` (vsdx example)
//! regenerates them deterministically from the local, untracked
//! `Sample/Office16/Visio Content/1033` install.

use std::path::PathBuf;

/// One catalog entry: embedded synthetic stencil plus its menu place.
pub struct BuiltinStencil {
    /// Stable id — file stem of the asset and the `builtin/<slug>.vssx`
    /// dedupe pseudo-path.
    pub slug: &'static str,
    /// More Shapes menu category (menu order follows first appearance).
    pub category: &'static str,
    /// Pane entry / menu title.
    pub title: &'static str,
    /// Default variant (US units; a metric-only entry's metric bytes).
    pub bytes: &'static [u8],
    /// The metric twin (`<slug>-m.vssx`), when that variant ships —
    /// the U/M pairs differ in master geometry (round inches vs round
    /// millimetres) and carry no file-level units marker, so which one
    /// opens is the app's call (#85 follow-up, 2026-10-27).
    pub metric_bytes: Option<&'static [u8]>,
}

impl BuiltinStencil {
    /// The bytes to open: the metric twin when asked for and present,
    /// else the default variant (and vice versa).
    pub fn bytes_for(&self, metric: bool) -> &'static [u8] {
        match (metric, self.metric_bytes) {
            (true, Some(m)) => m,
            (true, None) => self.bytes,
            (false, _) => self.bytes,
        }
    }
}

/// The catalog, in manifest order
/// (`assets/stencils/manifest.txt` — regenerate this table alongside
/// it; the digest test fails if the two drift apart).
pub const BUILTINS: &[BuiltinStencil] = &[
    BuiltinStencil {
        slug: "annot",
        category: "General",
        title: "Annotations",
        bytes: include_bytes!("../../assets/stencils/annot.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/annot-m.vssx")),
    },
    BuiltinStencil {
        slug: "arrows",
        category: "General",
        title: "Arrow Shapes",
        bytes: include_bytes!("../../assets/stencils/arrows.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/arrows-m.vssx")),
    },
    BuiltinStencil {
        slug: "basic",
        category: "General",
        title: "Basic Shapes",
        bytes: include_bytes!("../../assets/stencils/basic.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/basic-m.vssx")),
    },
    BuiltinStencil {
        slug: "block",
        category: "General",
        title: "Blocks",
        bytes: include_bytes!("../../assets/stencils/block.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/block-m.vssx")),
    },
    BuiltinStencil {
        slug: "block3",
        category: "General",
        title: "Blocks Raised",
        bytes: include_bytes!("../../assets/stencils/block3.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/block3-m.vssx")),
    },
    BuiltinStencil {
        slug: "blockp",
        category: "General",
        title: "Blocks with Perspective",
        bytes: include_bytes!("../../assets/stencils/blockp.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/blockp-m.vssx")),
    },
    BuiltinStencil {
        slug: "chart",
        category: "General",
        title: "Charting Shapes",
        bytes: include_bytes!("../../assets/stencils/chart.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/chart-m.vssx")),
    },
    BuiltinStencil {
        slug: "connec",
        category: "General",
        title: "Connectors",
        bytes: include_bytes!("../../assets/stencils/connec.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/connec-m.vssx")),
    },
    BuiltinStencil {
        slug: "linpat",
        category: "General",
        title: "Custom Line Patterns",
        bytes: include_bytes!("../../assets/stencils/linpat.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/linpat-m.vssx")),
    },
    BuiltinStencil {
        slug: "ptscld",
        category: "General",
        title: "Custom Patterns - Scaled",
        bytes: include_bytes!("../../assets/stencils/ptscld.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/ptscld-m.vssx")),
    },
    BuiltinStencil {
        slug: "ptuscl",
        category: "General",
        title: "Custom Patterns - Unscaled",
        bytes: include_bytes!("../../assets/stencils/ptuscl.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/ptuscl-m.vssx")),
    },
    BuiltinStencil {
        slug: "ba_dec",
        category: "General",
        title: "Decorative Shapes",
        bytes: include_bytes!("../../assets/stencils/ba_dec.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/ba_dec-m.vssx")),
    },
    BuiltinStencil {
        slug: "drawtl",
        category: "General",
        title: "Drawing Tool Shapes",
        bytes: include_bytes!("../../assets/stencils/drawtl.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/drawtl-m.vssx")),
    },
    BuiltinStencil {
        slug: "embell",
        category: "General",
        title: "Embellishments",
        bytes: include_bytes!("../../assets/stencils/embell.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/embell-m.vssx")),
    },
    BuiltinStencil {
        slug: "ba_grp",
        category: "General",
        title: "Graphs and Math Shapes",
        bytes: include_bytes!("../../assets/stencils/ba_grp.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/ba_grp-m.vssx")),
    },
    BuiltinStencil {
        slug: "dgicon",
        category: "General",
        title: "Icon Sets",
        bytes: include_bytes!("../../assets/stencils/dgicon.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/dgicon-m.vssx")),
    },
    BuiltinStencil {
        slug: "drilld",
        category: "General",
        title: "PivotDiagram Shapes",
        bytes: include_bytes!("../../assets/stencils/drilld.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/drilld-m.vssx")),
    },
    BuiltinStencil {
        slug: "symbol",
        category: "General",
        title: "Symbols",
        bytes: include_bytes!("../../assets/stencils/symbol.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/symbol-m.vssx")),
    },
    BuiltinStencil {
        slug: "bckgrn",
        category: "Accessories",
        title: "Backgrounds Gallery",
        bytes: include_bytes!("../../assets/stencils/bckgrn.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/bckgrn-m.vssx")),
    },
    BuiltinStencil {
        slug: "bordrs",
        category: "Accessories",
        title: "Borders & Titles Gallery",
        bytes: include_bytes!("../../assets/stencils/bordrs.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/bordrs-m.vssx")),
    },
    BuiltinStencil {
        slug: "calout",
        category: "Accessories",
        title: "Callouts",
        bytes: include_bytes!("../../assets/stencils/calout.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/calout-m.vssx")),
    },
    BuiltinStencil {
        slug: "sdcont",
        category: "Accessories",
        title: "Containers",
        bytes: include_bytes!("../../assets/stencils/sdcont.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/sdcont-m.vssx")),
    },
    BuiltinStencil {
        slug: "lgnd",
        category: "Accessories",
        title: "Legend",
        bytes: include_bytes!("../../assets/stencils/lgnd.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/lgnd-m.vssx")),
    },
    BuiltinStencil {
        slug: "audit",
        category: "Flowchart",
        title: "Audit Diagram Shapes",
        bytes: include_bytes!("../../assets/stencils/audit.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/audit-m.vssx")),
    },
    BuiltinStencil {
        slug: "basflo",
        category: "Flowchart",
        title: "Basic Flowchart Shapes",
        bytes: include_bytes!("../../assets/stencils/basflo.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/basflo-m.vssx")),
    },
    BuiltinStencil {
        slug: "bpmn",
        category: "Flowchart",
        title: "BPMN Shapes",
        bytes: include_bytes!("../../assets/stencils/bpmn.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/bpmn-m.vssx")),
    },
    BuiltinStencil {
        slug: "causef",
        category: "Flowchart",
        title: "Cause and Effect Diagram Shapes",
        bytes: include_bytes!("../../assets/stencils/causef.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/causef-m.vssx")),
    },
    BuiltinStencil {
        slug: "xfunc",
        category: "Flowchart",
        title: "Cross-Functional Flowchart Shapes",
        bytes: include_bytes!("../../assets/stencils/xfunc.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/xfunc-m.vssx")),
    },
    BuiltinStencil {
        slug: "epc",
        category: "Flowchart",
        title: "EPC Diagram Shapes",
        bytes: include_bytes!("../../assets/stencils/epc.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/epc-m.vssx")),
    },
    BuiltinStencil {
        slug: "fault",
        category: "Flowchart",
        title: "Fault Tree Analysis Shapes",
        bytes: include_bytes!("../../assets/stencils/fault.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/fault-m.vssx")),
    },
    BuiltinStencil {
        slug: "metro",
        category: "Flowchart",
        title: "Metro Shapes",
        bytes: include_bytes!("../../assets/stencils/metro.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/metro-m.vssx")),
    },
    BuiltinStencil {
        slug: "floshp",
        category: "Flowchart",
        title: "Miscellaneous Flowchart Shapes",
        bytes: include_bytes!("../../assets/stencils/floshp.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/floshp-m.vssx")),
    },
    BuiltinStencil {
        slug: "peannt",
        category: "Flowchart",
        title: "Process Annotations",
        bytes: include_bytes!("../../assets/stencils/peannt.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/peannt-m.vssx")),
    },
    BuiltinStencil {
        slug: "sdl",
        category: "Flowchart",
        title: "SDL Diagram Shapes",
        bytes: include_bytes!("../../assets/stencils/sdl.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/sdl-m.vssx")),
    },
    BuiltinStencil {
        slug: "ssflow",
        category: "Flowchart",
        title: "Six Sigma Flowchart Shapes",
        bytes: include_bytes!("../../assets/stencils/ssflow.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/ssflow-m.vssx")),
    },
    BuiltinStencil {
        slug: "sshoq",
        category: "Flowchart",
        title: "Six Sigma House of Quality Shapes",
        bytes: include_bytes!("../../assets/stencils/sshoq.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/sshoq-m.vssx")),
    },
    BuiltinStencil {
        slug: "tqm",
        category: "Flowchart",
        title: "TQM Diagram Shapes",
        bytes: include_bytes!("../../assets/stencils/tqm.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/tqm-m.vssx")),
    },
    BuiltinStencil {
        slug: "vsm",
        category: "Flowchart",
        title: "Value Stream Map Shapes",
        bytes: include_bytes!("../../assets/stencils/vsm.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/vsm-m.vssx")),
    },
    BuiltinStencil {
        slug: "wrkflo",
        category: "Flowchart",
        title: "Work Flow Diagram Shapes",
        bytes: include_bytes!("../../assets/stencils/wrkflo.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/wrkflo-m.vssx")),
    },
    BuiltinStencil {
        slug: "ado",
        category: "Network",
        title: "Active Directory Objects",
        bytes: include_bytes!("../../assets/stencils/ado.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/ado-m.vssx")),
    },
    BuiltinStencil {
        slug: "ads",
        category: "Network",
        title: "Active Directory Sites and Services",
        bytes: include_bytes!("../../assets/stencils/ads.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/ads-m.vssx")),
    },
    BuiltinStencil {
        slug: "compme",
        category: "Network",
        title: "Computers and Monitors",
        bytes: include_bytes!("../../assets/stencils/compme.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/compme-m.vssx")),
    },
    BuiltinStencil {
        slug: "dtlnet",
        category: "Network",
        title: "Detailed Network",
        bytes: include_bytes!("../../assets/stencils/dtlnet.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/dtlnet-m.vssx")),
    },
    BuiltinStencil {
        slug: "dtlnme",
        category: "Network",
        title: "Detailed Network Diagram",
        bytes: include_bytes!("../../assets/stencils/dtlnme.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/dtlnme-m.vssx")),
    },
    BuiltinStencil {
        slug: "excobj",
        category: "Network",
        title: "Exchange Objects",
        bytes: include_bytes!("../../assets/stencils/excobj.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/excobj-m.vssx")),
    },
    BuiltinStencil {
        slug: "freeqp",
        category: "Network",
        title: "Free-standing Rack Equipment",
        bytes: include_bytes!("../../assets/stencils/freeqp.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/freeqp-m.vssx")),
    },
    BuiltinStencil {
        slug: "ldapob",
        category: "Network",
        title: "LDAP Objects",
        bytes: include_bytes!("../../assets/stencils/ldapob.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/ldapob-m.vssx")),
    },
    BuiltinStencil {
        slug: "perime",
        category: "Network",
        title: "Network and Peripherals",
        bytes: include_bytes!("../../assets/stencils/perime.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/perime-m.vssx")),
    },
    BuiltinStencil {
        slug: "netloc",
        category: "Network",
        title: "Network Locations",
        bytes: include_bytes!("../../assets/stencils/netloc.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/netloc-m.vssx")),
    },
    BuiltinStencil {
        slug: "netrm",
        category: "Network",
        title: "Network Room Elements",
        bytes: include_bytes!("../../assets/stencils/netrm.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/netrm-m.vssx")),
    },
    BuiltinStencil {
        slug: "netsym",
        category: "Network",
        title: "Network Symbols",
        bytes: include_bytes!("../../assets/stencils/netsym.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/netsym-m.vssx")),
    },
    BuiltinStencil {
        slug: "periph",
        category: "Network",
        title: "Networks and Peripherals",
        bytes: include_bytes!("../../assets/stencils/periph.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/periph-m.vssx")),
    },
    BuiltinStencil {
        slug: "rcksme",
        category: "Network",
        title: "Rack Mounted Servers",
        bytes: include_bytes!("../../assets/stencils/rcksme.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/rcksme-m.vssx")),
    },
    BuiltinStencil {
        slug: "rcksvr",
        category: "Network",
        title: "Rack Mounted Servers (US Units)",
        bytes: include_bytes!("../../assets/stencils/rcksvr.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/rcksvr-m.vssx")),
    },
    BuiltinStencil {
        slug: "rckeqp",
        category: "Network",
        title: "Rack-mounted Equipment",
        bytes: include_bytes!("../../assets/stencils/rckeqp.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/rckeqp-m.vssx")),
    },
    BuiltinStencil {
        slug: "server",
        category: "Network",
        title: "Servers",
        bytes: include_bytes!("../../assets/stencils/server.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/server-m.vssx")),
    },
    BuiltinStencil {
        slug: "spwa",
        category: "Software and Database",
        title: "Actions - SharePoint 2010 Workflow",
        bytes: include_bytes!("../../assets/stencils/spwa.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/spwa-m.vssx")),
    },
    BuiltinStencil {
        slug: "spwa2",
        category: "Software and Database",
        title: "Actions - SharePoint 2013 Workflow",
        bytes: include_bytes!("../../assets/stencils/spwa2.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/spwa2-m.vssx")),
    },
    BuiltinStencil {
        slug: "dbchen",
        category: "Software and Database",
        title: "Chen’s Database Notation Shapes",
        bytes: include_bytes!("../../assets/stencils/dbchen.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/dbchen-m.vssx")),
    },
    BuiltinStencil {
        slug: "comole",
        category: "Software and Database",
        title: "COM and OLE",
        bytes: include_bytes!("../../assets/stencils/comole.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/comole-m.vssx")),
    },
    BuiltinStencil {
        slug: "wfcicn",
        category: "Software and Database",
        title: "Common Icons",
        bytes: include_bytes!("../../assets/stencils/wfcicn.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/wfcicn-m.vssx")),
    },
    BuiltinStencil {
        slug: "websit",
        category: "Software and Database",
        title: "Conceptual Web Site Shapes",
        bytes: include_bytes!("../../assets/stencils/websit.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/websit-m.vssx")),
    },
    BuiltinStencil {
        slug: "spwc",
        category: "Software and Database",
        title: "Conditions - SharePoint 2010 Workflow",
        bytes: include_bytes!("../../assets/stencils/spwc.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/spwc-m.vssx")),
    },
    BuiltinStencil {
        slug: "spwc2",
        category: "Software and Database",
        title: "Conditions - SharePoint 2013 Workflow",
        bytes: include_bytes!("../../assets/stencils/spwc2.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/spwc2-m.vssx")),
    },
    BuiltinStencil {
        slug: "spwt2",
        category: "Software and Database",
        title: "Containers & Terminators - SharePoint 2013 Workflow",
        bytes: include_bytes!("../../assets/stencils/spwt2.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/spwt2-m.vssx")),
    },
    BuiltinStencil {
        slug: "wfctrl",
        category: "Software and Database",
        title: "Controls",
        bytes: include_bytes!("../../assets/stencils/wfctrl.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/wfctrl-m.vssx")),
    },
    BuiltinStencil {
        slug: "dbcrow",
        category: "Software and Database",
        title: "Crow’s Foot Database Notation Shapes",
        bytes: include_bytes!("../../assets/stencils/dbcrow.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/dbcrow-m.vssx")),
    },
    BuiltinStencil {
        slug: "wfcrs",
        category: "Software and Database",
        title: "Cursors",
        bytes: include_bytes!("../../assets/stencils/wfcrs.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/wfcrs-m.vssx")),
    },
    BuiltinStencil {
        slug: "datflo",
        category: "Software and Database",
        title: "Data Flow Diagram Shapes",
        bytes: include_bytes!("../../assets/stencils/datflo.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/datflo-m.vssx")),
    },
    BuiltinStencil {
        slug: "wfdeme",
        category: "Software and Database",
        title: "Department",
        bytes: include_bytes!("../../assets/stencils/wfdeme.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/wfdeme-m.vssx")),
    },
    BuiltinStencil {
        slug: "entapp",
        category: "Software and Database",
        title: "Enterprise Application",
        bytes: include_bytes!("../../assets/stencils/entapp.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/entapp-m.vssx")),
    },
    BuiltinStencil {
        slug: "wfdlgs",
        category: "Software and Database",
        title: "Forms and Dialogs",
        bytes: include_bytes!("../../assets/stencils/wfdlgs.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/wfdlgs-m.vssx")),
    },
    BuiltinStencil {
        slug: "ganesa",
        category: "Software and Database",
        title: "Gane-Sarson",
        bytes: include_bytes!("../../assets/stencils/ganesa.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/ganesa-m.vssx")),
    },
    BuiltinStencil {
        slug: "idef0",
        category: "Software and Database",
        title: "IDEF0 Diagram Shapes",
        bytes: include_bytes!("../../assets/stencils/idef0.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/idef0-m.vssx")),
    },
    BuiltinStencil {
        slug: "dbidef1x",
        category: "Software and Database",
        title: "IDEF1X Database Notation Shapes",
        bytes: include_bytes!("../../assets/stencils/dbidef1x.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/dbidef1x-m.vssx")),
    },
    BuiltinStencil {
        slug: "langlv",
        category: "Software and Database",
        title: "Language Level Shapes",
        bytes: include_bytes!("../../assets/stencils/langlv.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/langlv-m.vssx")),
    },
    BuiltinStencil {
        slug: "memobj",
        category: "Software and Database",
        title: "Memory Objects",
        bytes: include_bytes!("../../assets/stencils/memobj.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/memobj-m.vssx")),
    },
    BuiltinStencil {
        slug: "spwt",
        category: "Software and Database",
        title: "Terminators - SharePoint 2010 Workflow",
        bytes: include_bytes!("../../assets/stencils/spwt.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/spwt-m.vssx")),
    },
    BuiltinStencil {
        slug: "wftlbr",
        category: "Software and Database",
        title: "Toolbars and Menus",
        bytes: include_bytes!("../../assets/stencils/wftlbr.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/wftlbr-m.vssx")),
    },
    BuiltinStencil {
        slug: "uactme",
        category: "Software and Database",
        title: "UML Activity Diagram Shapes",
        bytes: include_bytes!("../../assets/stencils/uactme.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/uactme-m.vssx")),
    },
    BuiltinStencil {
        slug: "ustrme",
        category: "Software and Database",
        title: "UML Class Diagram Shapes",
        bytes: include_bytes!("../../assets/stencils/ustrme.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/ustrme-m.vssx")),
    },
    BuiltinStencil {
        slug: "dbuml",
        category: "Software and Database",
        title: "UML Database Notation",
        bytes: include_bytes!("../../assets/stencils/dbuml.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/dbuml-m.vssx")),
    },
    BuiltinStencil {
        slug: "useqme",
        category: "Software and Database",
        title: "UML Sequence Diagram Shapes",
        bytes: include_bytes!("../../assets/stencils/useqme.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/useqme-m.vssx")),
    },
    BuiltinStencil {
        slug: "ustame",
        category: "Software and Database",
        title: "UML State Machine Shapes",
        bytes: include_bytes!("../../assets/stencils/ustame.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/ustame-m.vssx")),
    },
    BuiltinStencil {
        slug: "uuseme",
        category: "Software and Database",
        title: "UML Use Case Shapes",
        bytes: include_bytes!("../../assets/stencils/uuseme.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/uuseme-m.vssx")),
    },
    BuiltinStencil {
        slug: "wfwicn",
        category: "Software and Database",
        title: "Web and Media Icons",
        bytes: include_bytes!("../../assets/stencils/wfwicn.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/wfwicn-m.vssx")),
    },
    BuiltinStencil {
        slug: "webmap",
        category: "Software and Database",
        title: "Web Site Map Shapes (US units)",
        bytes: include_bytes!("../../assets/stencils/webmap.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/webmap-m.vssx")),
    },
    BuiltinStencil {
        slug: "wfobme",
        category: "Software and Database",
        title: "Work Flow Objects",
        bytes: include_bytes!("../../assets/stencils/wfobme.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/wfobme-m.vssx")),
    },
    BuiltinStencil {
        slug: "wfstme",
        category: "Software and Database",
        title: "Work Flow Steps",
        bytes: include_bytes!("../../assets/stencils/wfstme.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/wfstme-m.vssx")),
    },
    BuiltinStencil {
        slug: "wfdep",
        category: "Software and Database",
        title: "Workflow Departments",
        bytes: include_bytes!("../../assets/stencils/wfdep.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/wfdep-m.vssx")),
    },
    BuiltinStencil {
        slug: "wfobj",
        category: "Software and Database",
        title: "Workflow Objects",
        bytes: include_bytes!("../../assets/stencils/wfobj.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/wfobj-m.vssx")),
    },
    BuiltinStencil {
        slug: "wfstep",
        category: "Software and Database",
        title: "Workflow Steps",
        bytes: include_bytes!("../../assets/stencils/wfstep.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/wfstep-m.vssx")),
    },
    BuiltinStencil {
        slug: "alarm",
        category: "Maps and Floor Plans",
        title: "Alarm and Access Control",
        bytes: include_bytes!("../../assets/stencils/alarm.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/alarm-m.vssx")),
    },
    BuiltinStencil {
        slug: "appl",
        category: "Maps and Floor Plans",
        title: "Appliances",
        bytes: include_bytes!("../../assets/stencils/appl.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/appl-m.vssx")),
    },
    BuiltinStencil {
        slug: "bthkt",
        category: "Maps and Floor Plans",
        title: "Bath And Kitchen Plan",
        bytes: include_bytes!("../../assets/stencils/bthkt.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/bthkt-m.vssx")),
    },
    BuiltinStencil {
        slug: "bldcor",
        category: "Maps and Floor Plans",
        title: "Building Core",
        bytes: include_bytes!("../../assets/stencils/bldcor.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/bldcor-m.vssx")),
    },
    BuiltinStencil {
        slug: "cabnt",
        category: "Maps and Floor Plans",
        title: "Cabinets",
        bytes: include_bytes!("../../assets/stencils/cabnt.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/cabnt-m.vssx")),
    },
    BuiltinStencil {
        slug: "cubicl",
        category: "Maps and Floor Plans",
        title: "Cubicles",
        bytes: include_bytes!("../../assets/stencils/cubicl.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/cubicl-m.vssx")),
    },
    BuiltinStencil {
        slug: "dimarc",
        category: "Maps and Floor Plans",
        title: "Dimensioning - Architectural",
        bytes: include_bytes!("../../assets/stencils/dimarc.vssx"),
        metric_bytes: None,
    },
    BuiltinStencil {
        slug: "map3d",
        category: "Maps and Floor Plans",
        title: "Directional Map Shapes 3D",
        bytes: include_bytes!("../../assets/stencils/map3d.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/map3d-m.vssx")),
    },
    BuiltinStencil {
        slug: "eletel",
        category: "Maps and Floor Plans",
        title: "Electrical and Telecom",
        bytes: include_bytes!("../../assets/stencils/eletel.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/eletel-m.vssx")),
    },
    BuiltinStencil {
        slug: "furn",
        category: "Maps and Floor Plans",
        title: "Furniture",
        bytes: include_bytes!("../../assets/stencils/furn.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/furn-m.vssx")),
    },
    BuiltinStencil {
        slug: "garden",
        category: "Maps and Floor Plans",
        title: "Garden Accessories",
        bytes: include_bytes!("../../assets/stencils/garden.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/garden-m.vssx")),
    },
    BuiltinStencil {
        slug: "hvacc",
        category: "Maps and Floor Plans",
        title: "HVAC Controls",
        bytes: include_bytes!("../../assets/stencils/hvacc.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/hvacc-m.vssx")),
    },
    BuiltinStencil {
        slug: "hvacce",
        category: "Maps and Floor Plans",
        title: "HVAC Controls Equipment",
        bytes: include_bytes!("../../assets/stencils/hvacce.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/hvacce-m.vssx")),
    },
    BuiltinStencil {
        slug: "hvacd",
        category: "Maps and Floor Plans",
        title: "HVAC Ductwork",
        bytes: include_bytes!("../../assets/stencils/hvacd.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/hvacd-m.vssx")),
    },
    BuiltinStencil {
        slug: "hvaceq",
        category: "Maps and Floor Plans",
        title: "HVAC Equipment",
        bytes: include_bytes!("../../assets/stencils/hvaceq.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/hvaceq-m.vssx")),
    },
    BuiltinStencil {
        slug: "irrig",
        category: "Maps and Floor Plans",
        title: "Irrigation",
        bytes: include_bytes!("../../assets/stencils/irrig.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/irrig-m.vssx")),
    },
    BuiltinStencil {
        slug: "lndmrk",
        category: "Maps and Floor Plans",
        title: "Landmark Shapes",
        bytes: include_bytes!("../../assets/stencils/lndmrk.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/lndmrk-m.vssx")),
    },
    BuiltinStencil {
        slug: "offacc",
        category: "Maps and Floor Plans",
        title: "Office Accessories",
        bytes: include_bytes!("../../assets/stencils/offacc.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/offacc-m.vssx")),
    },
    BuiltinStencil {
        slug: "offeqp",
        category: "Maps and Floor Plans",
        title: "Office Equipment",
        bytes: include_bytes!("../../assets/stencils/offeqp.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/offeqp-m.vssx")),
    },
    BuiltinStencil {
        slug: "offfrn",
        category: "Maps and Floor Plans",
        title: "Office Furniture",
        bytes: include_bytes!("../../assets/stencils/offfrn.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/offfrn-m.vssx")),
    },
    BuiltinStencil {
        slug: "prkrd",
        category: "Maps and Floor Plans",
        title: "Parking and Roads",
        bytes: include_bytes!("../../assets/stencils/prkrd.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/prkrd-m.vssx")),
    },
    BuiltinStencil {
        slug: "pipe1",
        category: "Maps and Floor Plans",
        title: "Pipes and Valves - Pipes 1",
        bytes: include_bytes!("../../assets/stencils/pipe1.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/pipe1-m.vssx")),
    },
    BuiltinStencil {
        slug: "pipe2",
        category: "Maps and Floor Plans",
        title: "Pipes and Valves - Pipes 2",
        bytes: include_bytes!("../../assets/stencils/pipe2.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/pipe2-m.vssx")),
    },
    BuiltinStencil {
        slug: "valve1",
        category: "Maps and Floor Plans",
        title: "Pipes and Valves - Valves 1",
        bytes: include_bytes!("../../assets/stencils/valve1.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/valve1-m.vssx")),
    },
    BuiltinStencil {
        slug: "valve2",
        category: "Maps and Floor Plans",
        title: "Pipes and Valves - Valves 2",
        bytes: include_bytes!("../../assets/stencils/valve2.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/valve2-m.vssx")),
    },
    BuiltinStencil {
        slug: "plant",
        category: "Maps and Floor Plans",
        title: "Planting",
        bytes: include_bytes!("../../assets/stencils/plant.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/plant-m.vssx")),
    },
    BuiltinStencil {
        slug: "plumb",
        category: "Maps and Floor Plans",
        title: "Plumbing",
        bytes: include_bytes!("../../assets/stencils/plumb.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/plumb-m.vssx")),
    },
    BuiltinStencil {
        slug: "ptsint",
        category: "Maps and Floor Plans",
        title: "Points Of Interest",
        bytes: include_bytes!("../../assets/stencils/ptsint.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/ptsint-m.vssx")),
    },
    BuiltinStencil {
        slug: "recshp",
        category: "Maps and Floor Plans",
        title: "Recreation Shapes",
        bytes: include_bytes!("../../assets/stencils/recshp.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/recshp-m.vssx")),
    },
    BuiltinStencil {
        slug: "regstr",
        category: "Maps and Floor Plans",
        title: "Registers, Grilles and Diffusers",
        bytes: include_bytes!("../../assets/stencils/regstr.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/regstr-m.vssx")),
    },
    BuiltinStencil {
        slug: "road",
        category: "Maps and Floor Plans",
        title: "Road Shapes",
        bytes: include_bytes!("../../assets/stencils/road.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/road-m.vssx")),
    },
    BuiltinStencil {
        slug: "shpmch",
        category: "Maps and Floor Plans",
        title: "Shop Floor - Machines and Equipment",
        bytes: include_bytes!("../../assets/stencils/shpmch.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/shpmch-m.vssx")),
    },
    BuiltinStencil {
        slug: "shpstr",
        category: "Maps and Floor Plans",
        title: "Shop Floor - Storage and Distribution",
        bytes: include_bytes!("../../assets/stencils/shpstr.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/shpstr-m.vssx")),
    },
    BuiltinStencil {
        slug: "sitacc",
        category: "Maps and Floor Plans",
        title: "Site Accessories",
        bytes: include_bytes!("../../assets/stencils/sitacc.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/sitacc-m.vssx")),
    },
    BuiltinStencil {
        slug: "recrt",
        category: "Maps and Floor Plans",
        title: "Sport Fields and Recreation",
        bytes: include_bytes!("../../assets/stencils/recrt.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/recrt-m.vssx")),
    },
    BuiltinStencil {
        slug: "transp",
        category: "Maps and Floor Plans",
        title: "Transportation Shapes",
        bytes: include_bytes!("../../assets/stencils/transp.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/transp-m.vssx")),
    },
    BuiltinStencil {
        slug: "vehicl",
        category: "Maps and Floor Plans",
        title: "Vehicles",
        bytes: include_bytes!("../../assets/stencils/vehicl.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/vehicl-m.vssx")),
    },
    BuiltinStencil {
        slug: "vidsur",
        category: "Maps and Floor Plans",
        title: "Video Surveillance",
        bytes: include_bytes!("../../assets/stencils/vidsur.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/vidsur-m.vssx")),
    },
    BuiltinStencil {
        slug: "wadowi",
        category: "Maps and Floor Plans",
        title: "Walls, Doors and Windows",
        bytes: include_bytes!("../../assets/stencils/wadowi.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/wadowi-m.vssx")),
    },
    BuiltinStencil {
        slug: "wall",
        category: "Maps and Floor Plans",
        title: "Walls, Shell and Structure",
        bytes: include_bytes!("../../assets/stencils/wall.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/wall-m.vssx")),
    },
    BuiltinStencil {
        slug: "shprec",
        category: "Maps and Floor Plans",
        title: "Warehouse - Shipping and Receiving",
        bytes: include_bytes!("../../assets/stencils/shprec.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/shprec-m.vssx")),
    },
    BuiltinStencil {
        slug: "logic",
        category: "Engineering",
        title: "Analog and Digital Logic",
        bytes: include_bytes!("../../assets/stencils/logic.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/logic-m.vssx")),
    },
    BuiltinStencil {
        slug: "eecomp",
        category: "Engineering",
        title: "Composite Assemblies",
        bytes: include_bytes!("../../assets/stencils/eecomp.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/eecomp-m.vssx")),
    },
    BuiltinStencil {
        slug: "dimeng",
        category: "Engineering",
        title: "Dimensioning - Engineering",
        bytes: include_bytes!("../../assets/stencils/dimeng.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/dimeng-m.vssx")),
    },
    BuiltinStencil {
        slug: "peeqp",
        category: "Engineering",
        title: "Equipment - General",
        bytes: include_bytes!("../../assets/stencils/peeqp.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/peeqp-m.vssx")),
    },
    BuiltinStencil {
        slug: "peheat",
        category: "Engineering",
        title: "Equipment - Heat Exchangers",
        bytes: include_bytes!("../../assets/stencils/peheat.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/peheat-m.vssx")),
    },
    BuiltinStencil {
        slug: "pepump",
        category: "Engineering",
        title: "Equipment - Pumps",
        bytes: include_bytes!("../../assets/stencils/pepump.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/pepump-m.vssx")),
    },
    BuiltinStencil {
        slug: "pevess",
        category: "Engineering",
        title: "Equipment - Vessels",
        bytes: include_bytes!("../../assets/stencils/pevess.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/pevess-m.vssx")),
    },
    BuiltinStencil {
        slug: "fastn1",
        category: "Engineering",
        title: "Fasteners 1",
        bytes: include_bytes!("../../assets/stencils/fastn1.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/fastn1-m.vssx")),
    },
    BuiltinStencil {
        slug: "fastn2",
        category: "Engineering",
        title: "Fasteners 2",
        bytes: include_bytes!("../../assets/stencils/fastn2.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/fastn2-m.vssx")),
    },
    BuiltinStencil {
        slug: "fpeqp",
        category: "Engineering",
        title: "Fluid Power - Equipment",
        bytes: include_bytes!("../../assets/stencils/fpeqp.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/fpeqp-m.vssx")),
    },
    BuiltinStencil {
        slug: "fpassm",
        category: "Engineering",
        title: "Fluid Power - Valve Assembly",
        bytes: include_bytes!("../../assets/stencils/fpassm.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/fpassm-m.vssx")),
    },
    BuiltinStencil {
        slug: "fpvalv",
        category: "Engineering",
        title: "Fluid Power - Valves",
        bytes: include_bytes!("../../assets/stencils/fpvalv.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/fpvalv-m.vssx")),
    },
    BuiltinStencil {
        slug: "eefund",
        category: "Engineering",
        title: "Fundamental Items",
        bytes: include_bytes!("../../assets/stencils/eefund.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/eefund-m.vssx")),
    },
    BuiltinStencil {
        slug: "gdt",
        category: "Engineering",
        title: "Geometric Dimensioning and Tolerancing",
        bytes: include_bytes!("../../assets/stencils/gdt.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/gdt-m.vssx")),
    },
    BuiltinStencil {
        slug: "intann",
        category: "Engineering",
        title: "Initiation and Annunciation",
        bytes: include_bytes!("../../assets/stencils/intann.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/intann-m.vssx")),
    },
    BuiltinStencil {
        slug: "peinst",
        category: "Engineering",
        title: "Instruments",
        bytes: include_bytes!("../../assets/stencils/peinst.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/peinst-m.vssx")),
    },
    BuiltinStencil {
        slug: "eechip",
        category: "Engineering",
        title: "Integrated Circuit Components",
        bytes: include_bytes!("../../assets/stencils/eechip.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/eechip-m.vssx")),
    },
    BuiltinStencil {
        slug: "eemain",
        category: "Engineering",
        title: "Maintenance Symbols",
        bytes: include_bytes!("../../assets/stencils/eemain.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/eemain-m.vssx")),
    },
    BuiltinStencil {
        slug: "eemaps",
        category: "Engineering",
        title: "Maps and Charts",
        bytes: include_bytes!("../../assets/stencils/eemaps.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/eemaps-m.vssx")),
    },
    BuiltinStencil {
        slug: "pepipe",
        category: "Engineering",
        title: "Pipelines",
        bytes: include_bytes!("../../assets/stencils/pepipe.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/pepipe-m.vssx")),
    },
    BuiltinStencil {
        slug: "eequal",
        category: "Engineering",
        title: "Qualifying Symbols",
        bytes: include_bytes!("../../assets/stencils/eequal.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/eequal-m.vssx")),
    },
    BuiltinStencil {
        slug: "eemech",
        category: "Engineering",
        title: "Rotating Equip and Mech Functions",
        bytes: include_bytes!("../../assets/stencils/eemech.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/eemech-m.vssx")),
    },
    BuiltinStencil {
        slug: "eesemi",
        category: "Engineering",
        title: "Semiconductors And Electron Tubes",
        bytes: include_bytes!("../../assets/stencils/eesemi.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/eesemi-m.vssx")),
    },
    BuiltinStencil {
        slug: "sprbr",
        category: "Engineering",
        title: "Springs and Bearings",
        bytes: include_bytes!("../../assets/stencils/sprbr.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/sprbr-m.vssx")),
    },
    BuiltinStencil {
        slug: "eeswch",
        category: "Engineering",
        title: "Switches And Relays",
        bytes: include_bytes!("../../assets/stencils/eeswch.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/eeswch-m.vssx")),
    },
    BuiltinStencil {
        slug: "eetcom",
        category: "Engineering",
        title: "Telecom Switch and Peripheral Equip",
        bytes: include_bytes!("../../assets/stencils/eetcom.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/eetcom-m.vssx")),
    },
    BuiltinStencil {
        slug: "eeterm",
        category: "Engineering",
        title: "Terminals and Connectors",
        bytes: include_bytes!("../../assets/stencils/eeterm.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/eeterm-m.vssx")),
    },
    BuiltinStencil {
        slug: "title",
        category: "Engineering",
        title: "Title Blocks",
        bytes: include_bytes!("../../assets/stencils/title.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/title-m.vssx")),
    },
    BuiltinStencil {
        slug: "eetran",
        category: "Engineering",
        title: "Transformers and Windings",
        bytes: include_bytes!("../../assets/stencils/eetran.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/eetran-m.vssx")),
    },
    BuiltinStencil {
        slug: "eepath",
        category: "Engineering",
        title: "Transmission Paths",
        bytes: include_bytes!("../../assets/stencils/eepath.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/eepath-m.vssx")),
    },
    BuiltinStencil {
        slug: "pevalv",
        category: "Engineering",
        title: "Valves and Fittings",
        bytes: include_bytes!("../../assets/stencils/pevalv.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/pevalv-m.vssx")),
    },
    BuiltinStencil {
        slug: "eevhf",
        category: "Engineering",
        title: "VHF-UHF-SHF",
        bytes: include_bytes!("../../assets/stencils/eevhf.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/eevhf-m.vssx")),
    },
    BuiltinStencil {
        slug: "weld",
        category: "Engineering",
        title: "Welding Symbols",
        bytes: include_bytes!("../../assets/stencils/weld.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/weld-m.vssx")),
    },
    BuiltinStencil {
        slug: "calndr",
        category: "Schedule",
        title: "Calendar Shapes (US units)",
        bytes: include_bytes!("../../assets/stencils/calndr.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/calndr-m.vssx")),
    },
    BuiltinStencil {
        slug: "gantt",
        category: "Schedule",
        title: "Gantt Chart Shapes",
        bytes: include_bytes!("../../assets/stencils/gantt.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/gantt-m.vssx")),
    },
    BuiltinStencil {
        slug: "pert",
        category: "Schedule",
        title: "PERT Chart Shapes",
        bytes: include_bytes!("../../assets/stencils/pert.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/pert-m.vssx")),
    },
    BuiltinStencil {
        slug: "timeln",
        category: "Schedule",
        title: "Timeline Shapes",
        bytes: include_bytes!("../../assets/stencils/timeln.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/timeln-m.vssx")),
    },
    BuiltinStencil {
        slug: "orgblt",
        category: "Organization Chart",
        title: "Belt - Organization Chart Shapes",
        bytes: include_bytes!("../../assets/stencils/orgblt.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/orgblt-m.vssx")),
    },
    BuiltinStencil {
        slug: "orgbnd",
        category: "Organization Chart",
        title: "Bound - Organization Chart Shapes",
        bytes: include_bytes!("../../assets/stencils/orgbnd.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/orgbnd-m.vssx")),
    },
    BuiltinStencil {
        slug: "orgcns",
        category: "Organization Chart",
        title: "Coin - Organization Chart Shapes",
        bytes: include_bytes!("../../assets/stencils/orgcns.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/orgcns-m.vssx")),
    },
    BuiltinStencil {
        slug: "orgnch",
        category: "Organization Chart",
        title: "Notch - Organization Chart Shapes",
        bytes: include_bytes!("../../assets/stencils/orgnch.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/orgnch-m.vssx")),
    },
    BuiltinStencil {
        slug: "orgch",
        category: "Organization Chart",
        title: "Organization Chart Shapes",
        bytes: include_bytes!("../../assets/stencils/orgch.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/orgch-m.vssx")),
    },
    BuiltinStencil {
        slug: "orgpan",
        category: "Organization Chart",
        title: "Panel - Organization Chart Shapes",
        bytes: include_bytes!("../../assets/stencils/orgpan.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/orgpan-m.vssx")),
    },
    BuiltinStencil {
        slug: "orgper",
        category: "Organization Chart",
        title: "Perspective - Organization Chart Shapes",
        bytes: include_bytes!("../../assets/stencils/orgper.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/orgper-m.vssx")),
    },
    BuiltinStencil {
        slug: "orgpet",
        category: "Organization Chart",
        title: "Petal - Organization Chart Shapes",
        bytes: include_bytes!("../../assets/stencils/orgpet.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/orgpet-m.vssx")),
    },
    BuiltinStencil {
        slug: "orgpip",
        category: "Organization Chart",
        title: "Pip - Organization Chart Shapes",
        bytes: include_bytes!("../../assets/stencils/orgpip.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/orgpip-m.vssx")),
    },
    BuiltinStencil {
        slug: "orgtac",
        category: "Organization Chart",
        title: "Shapetacular - Organization Chart Shapes",
        bytes: include_bytes!("../../assets/stencils/orgtac.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/orgtac-m.vssx")),
    },
    BuiltinStencil {
        slug: "orgsto",
        category: "Organization Chart",
        title: "Stone - Organization Chart Shapes",
        bytes: include_bytes!("../../assets/stencils/orgsto.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/orgsto-m.vssx")),
    },
    BuiltinStencil {
        slug: "bstorm",
        category: "Business",
        title: "Brainstorming",
        bytes: include_bytes!("../../assets/stencils/bstorm.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/bstorm-m.vssx")),
    },
    BuiltinStencil {
        slug: "compln",
        category: "Business",
        title: "Compliance Shapes",
        bytes: include_bytes!("../../assets/stencils/compln.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/compln-m.vssx")),
    },
    BuiltinStencil {
        slug: "itil",
        category: "Business",
        title: "ITIL Shapes",
        bytes: include_bytes!("../../assets/stencils/itil.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/itil-m.vssx")),
    },
    BuiltinStencil {
        slug: "mrktdg",
        category: "Business",
        title: "Marketing Diagrams",
        bytes: include_bytes!("../../assets/stencils/mrktdg.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/mrktdg-m.vssx")),
    },
    BuiltinStencil {
        slug: "bpres",
        category: "Business",
        title: "Resources",
        bytes: include_bytes!("../../assets/stencils/bpres.vssx"),
        metric_bytes: Some(include_bytes!("../../assets/stencils/bpres-m.vssx")),
    },
];

pub fn by_slug(slug: &str) -> Option<&'static BuiltinStencil> {
    BUILTINS.iter().find(|b| b.slug == slug)
}

/// Categories in menu order (unique, first appearance).
pub fn categories() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = Vec::new();
    for b in BUILTINS {
        if !v.contains(&b.category) {
            v.push(b.category);
        }
    }
    v
}

pub fn by_category(category: &'static str) -> impl Iterator<Item = &'static BuiltinStencil> {
    BUILTINS.iter().filter(move |b| b.category == category)
}

/// Dedupe key for an open built-in: a `builtin/…` pseudo-path that
/// never exists on disk, so it can never collide with a file stencil
/// and canonicalization leaves it verbatim.
pub fn source_path(slug: &str) -> PathBuf {
    PathBuf::from(format!("builtin/{slug}.vssx"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The committed assets are exactly what the extractor produced
    /// (#87; both unit variants since the #85 follow-up): every asset
    /// parses as a stencil, has visible masters, and its embedded
    /// bytes hash to the recorded digest. `digests.txt` is generated
    /// by `extract-stencils.rs`; editing an asset without re-running
    /// the extraction fails here.
    #[test]
    fn committed_assets_match_recorded_digests_and_parse() {
        let recorded: Vec<(String, String)> = include_str!("../../assets/stencils/digests.txt")
            .lines()
            .filter_map(|l| {
                let (hash, file) = l.split_once("  ")?;
                Some((hash.to_string(), file.to_string()))
            })
            .collect();
        let mut checked = 0usize;
        for b in BUILTINS {
            for (name, bytes) in [
                (format!("{}.vssx", b.slug), b.bytes),
                (format!("{}-m.vssx", b.slug), b.metric_bytes.unwrap_or(&[])),
            ] {
                if bytes.is_empty() {
                    continue;
                }
                let (hash, _) = recorded
                    .iter()
                    .find(|(_, f)| *f == name)
                    .unwrap_or_else(|| panic!("{name} missing from digests.txt"));
                use sha2::{Digest, Sha256};
                let mut h = Sha256::new();
                h.update(bytes);
                let got: String = h.finalize().iter().map(|x| format!("{x:02x}")).collect();
                assert_eq!(&got, hash, "{name} tampered or stale");
                let doc = vsdx::parse::parse_bytes(bytes)
                    .unwrap_or_else(|e| panic!("{name} failed to parse: {e:#}"));
                assert!(!doc.masters.is_empty(), "{name} carries masters");
                assert!(doc.pages.is_empty(), "{name} is a stencil (no pages)");
                checked += 1;
            }
        }
        assert_eq!(checked, recorded.len(), "no asset drifted from the manifest");
    }

    /// Variant selection (#85 follow-up): metric when asked for and
    /// present; fall back to the other variant when one is missing.
    #[test]
    fn bytes_for_picks_the_variant_with_fallback() {
        let basic = by_slug("basic").expect("basic ships");
        let us = basic.bytes.as_ptr();
        let metric = basic.metric_bytes.expect("basic has a metric twin").as_ptr();
        assert_eq!(basic.bytes_for(false).as_ptr(), us);
        assert_eq!(basic.bytes_for(true).as_ptr(), metric);
        // dimarc ships US-only: metric falls back to the default.
        let dimarc = by_slug("dimarc").expect("dimarc ships");
        assert!(dimarc.metric_bytes.is_none());
        assert_eq!(
            dimarc.bytes_for(true).as_ptr(),
            dimarc.bytes.as_ptr(),
            "US-only stencils fall back to the default variant"
        );
    }
}

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
    pub bytes: &'static [u8],
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
    },
    BuiltinStencil {
        slug: "arrows",
        category: "General",
        title: "Arrow Shapes",
        bytes: include_bytes!("../../assets/stencils/arrows.vssx"),
    },
    BuiltinStencil {
        slug: "basic",
        category: "General",
        title: "Basic Shapes",
        bytes: include_bytes!("../../assets/stencils/basic.vssx"),
    },
    BuiltinStencil {
        slug: "block",
        category: "General",
        title: "Blocks",
        bytes: include_bytes!("../../assets/stencils/block.vssx"),
    },
    BuiltinStencil {
        slug: "block3",
        category: "General",
        title: "Blocks Raised",
        bytes: include_bytes!("../../assets/stencils/block3.vssx"),
    },
    BuiltinStencil {
        slug: "blockp",
        category: "General",
        title: "Blocks with Perspective",
        bytes: include_bytes!("../../assets/stencils/blockp.vssx"),
    },
    BuiltinStencil {
        slug: "chart",
        category: "General",
        title: "Charting Shapes",
        bytes: include_bytes!("../../assets/stencils/chart.vssx"),
    },
    BuiltinStencil {
        slug: "connec",
        category: "General",
        title: "Connectors",
        bytes: include_bytes!("../../assets/stencils/connec.vssx"),
    },
    BuiltinStencil {
        slug: "linpat",
        category: "General",
        title: "Custom Line Patterns",
        bytes: include_bytes!("../../assets/stencils/linpat.vssx"),
    },
    BuiltinStencil {
        slug: "ptscld",
        category: "General",
        title: "Custom Patterns - Scaled",
        bytes: include_bytes!("../../assets/stencils/ptscld.vssx"),
    },
    BuiltinStencil {
        slug: "ptuscl",
        category: "General",
        title: "Custom Patterns - Unscaled",
        bytes: include_bytes!("../../assets/stencils/ptuscl.vssx"),
    },
    BuiltinStencil {
        slug: "ba_dec",
        category: "General",
        title: "Decorative Shapes",
        bytes: include_bytes!("../../assets/stencils/ba_dec.vssx"),
    },
    BuiltinStencil {
        slug: "drawtl",
        category: "General",
        title: "Drawing Tool Shapes",
        bytes: include_bytes!("../../assets/stencils/drawtl.vssx"),
    },
    BuiltinStencil {
        slug: "embell",
        category: "General",
        title: "Embellishments",
        bytes: include_bytes!("../../assets/stencils/embell.vssx"),
    },
    BuiltinStencil {
        slug: "ba_grp",
        category: "General",
        title: "Graphs and Math Shapes",
        bytes: include_bytes!("../../assets/stencils/ba_grp.vssx"),
    },
    BuiltinStencil {
        slug: "dgicon",
        category: "General",
        title: "Icon Sets",
        bytes: include_bytes!("../../assets/stencils/dgicon.vssx"),
    },
    BuiltinStencil {
        slug: "drilld",
        category: "General",
        title: "PivotDiagram Shapes",
        bytes: include_bytes!("../../assets/stencils/drilld.vssx"),
    },
    BuiltinStencil {
        slug: "symbol",
        category: "General",
        title: "Symbols",
        bytes: include_bytes!("../../assets/stencils/symbol.vssx"),
    },
    BuiltinStencil {
        slug: "bckgrn",
        category: "Accessories",
        title: "Backgrounds Gallery",
        bytes: include_bytes!("../../assets/stencils/bckgrn.vssx"),
    },
    BuiltinStencil {
        slug: "bordrs",
        category: "Accessories",
        title: "Borders & Titles Gallery",
        bytes: include_bytes!("../../assets/stencils/bordrs.vssx"),
    },
    BuiltinStencil {
        slug: "calout",
        category: "Accessories",
        title: "Callouts",
        bytes: include_bytes!("../../assets/stencils/calout.vssx"),
    },
    BuiltinStencil {
        slug: "sdcont",
        category: "Accessories",
        title: "Containers",
        bytes: include_bytes!("../../assets/stencils/sdcont.vssx"),
    },
    BuiltinStencil {
        slug: "lgnd",
        category: "Accessories",
        title: "Legend",
        bytes: include_bytes!("../../assets/stencils/lgnd.vssx"),
    },
    BuiltinStencil {
        slug: "audit",
        category: "Flowchart",
        title: "Audit Diagram Shapes",
        bytes: include_bytes!("../../assets/stencils/audit.vssx"),
    },
    BuiltinStencil {
        slug: "basflo",
        category: "Flowchart",
        title: "Basic Flowchart Shapes",
        bytes: include_bytes!("../../assets/stencils/basflo.vssx"),
    },
    BuiltinStencil {
        slug: "bpmn",
        category: "Flowchart",
        title: "BPMN Shapes",
        bytes: include_bytes!("../../assets/stencils/bpmn.vssx"),
    },
    BuiltinStencil {
        slug: "causef",
        category: "Flowchart",
        title: "Cause and Effect Diagram Shapes",
        bytes: include_bytes!("../../assets/stencils/causef.vssx"),
    },
    BuiltinStencil {
        slug: "xfunc",
        category: "Flowchart",
        title: "Cross-Functional Flowchart Shapes",
        bytes: include_bytes!("../../assets/stencils/xfunc.vssx"),
    },
    BuiltinStencil {
        slug: "epc",
        category: "Flowchart",
        title: "EPC Diagram Shapes",
        bytes: include_bytes!("../../assets/stencils/epc.vssx"),
    },
    BuiltinStencil {
        slug: "fault",
        category: "Flowchart",
        title: "Fault Tree Analysis Shapes",
        bytes: include_bytes!("../../assets/stencils/fault.vssx"),
    },
    BuiltinStencil {
        slug: "metro",
        category: "Flowchart",
        title: "Metro Shapes",
        bytes: include_bytes!("../../assets/stencils/metro.vssx"),
    },
    BuiltinStencil {
        slug: "floshp",
        category: "Flowchart",
        title: "Miscellaneous Flowchart Shapes",
        bytes: include_bytes!("../../assets/stencils/floshp.vssx"),
    },
    BuiltinStencil {
        slug: "peannt",
        category: "Flowchart",
        title: "Process Annotations",
        bytes: include_bytes!("../../assets/stencils/peannt.vssx"),
    },
    BuiltinStencil {
        slug: "sdl",
        category: "Flowchart",
        title: "SDL Diagram Shapes",
        bytes: include_bytes!("../../assets/stencils/sdl.vssx"),
    },
    BuiltinStencil {
        slug: "ssflow",
        category: "Flowchart",
        title: "Six Sigma Flowchart Shapes",
        bytes: include_bytes!("../../assets/stencils/ssflow.vssx"),
    },
    BuiltinStencil {
        slug: "sshoq",
        category: "Flowchart",
        title: "Six Sigma House of Quality Shapes",
        bytes: include_bytes!("../../assets/stencils/sshoq.vssx"),
    },
    BuiltinStencil {
        slug: "tqm",
        category: "Flowchart",
        title: "TQM Diagram Shapes",
        bytes: include_bytes!("../../assets/stencils/tqm.vssx"),
    },
    BuiltinStencil {
        slug: "vsm",
        category: "Flowchart",
        title: "Value Stream Map Shapes",
        bytes: include_bytes!("../../assets/stencils/vsm.vssx"),
    },
    BuiltinStencil {
        slug: "wrkflo",
        category: "Flowchart",
        title: "Work Flow Diagram Shapes",
        bytes: include_bytes!("../../assets/stencils/wrkflo.vssx"),
    },
    BuiltinStencil {
        slug: "ado",
        category: "Network",
        title: "Active Directory Objects",
        bytes: include_bytes!("../../assets/stencils/ado.vssx"),
    },
    BuiltinStencil {
        slug: "ads",
        category: "Network",
        title: "Active Directory Sites and Services",
        bytes: include_bytes!("../../assets/stencils/ads.vssx"),
    },
    BuiltinStencil {
        slug: "compme",
        category: "Network",
        title: "Computers and Monitors",
        bytes: include_bytes!("../../assets/stencils/compme.vssx"),
    },
    BuiltinStencil {
        slug: "dtlnet",
        category: "Network",
        title: "Detailed Network",
        bytes: include_bytes!("../../assets/stencils/dtlnet.vssx"),
    },
    BuiltinStencil {
        slug: "dtlnme",
        category: "Network",
        title: "Detailed Network Diagram",
        bytes: include_bytes!("../../assets/stencils/dtlnme.vssx"),
    },
    BuiltinStencil {
        slug: "excobj",
        category: "Network",
        title: "Exchange Objects",
        bytes: include_bytes!("../../assets/stencils/excobj.vssx"),
    },
    BuiltinStencil {
        slug: "freeqp",
        category: "Network",
        title: "Free-standing Rack Equipment",
        bytes: include_bytes!("../../assets/stencils/freeqp.vssx"),
    },
    BuiltinStencil {
        slug: "ldapob",
        category: "Network",
        title: "LDAP Objects",
        bytes: include_bytes!("../../assets/stencils/ldapob.vssx"),
    },
    BuiltinStencil {
        slug: "perime",
        category: "Network",
        title: "Network and Peripherals",
        bytes: include_bytes!("../../assets/stencils/perime.vssx"),
    },
    BuiltinStencil {
        slug: "netloc",
        category: "Network",
        title: "Network Locations",
        bytes: include_bytes!("../../assets/stencils/netloc.vssx"),
    },
    BuiltinStencil {
        slug: "netrm",
        category: "Network",
        title: "Network Room Elements",
        bytes: include_bytes!("../../assets/stencils/netrm.vssx"),
    },
    BuiltinStencil {
        slug: "netsym",
        category: "Network",
        title: "Network Symbols",
        bytes: include_bytes!("../../assets/stencils/netsym.vssx"),
    },
    BuiltinStencil {
        slug: "periph",
        category: "Network",
        title: "Networks and Peripherals",
        bytes: include_bytes!("../../assets/stencils/periph.vssx"),
    },
    BuiltinStencil {
        slug: "rcksme",
        category: "Network",
        title: "Rack Mounted Servers",
        bytes: include_bytes!("../../assets/stencils/rcksme.vssx"),
    },
    BuiltinStencil {
        slug: "rcksvr",
        category: "Network",
        title: "Rack Mounted Servers (US Units)",
        bytes: include_bytes!("../../assets/stencils/rcksvr.vssx"),
    },
    BuiltinStencil {
        slug: "rckeqp",
        category: "Network",
        title: "Rack-mounted Equipment",
        bytes: include_bytes!("../../assets/stencils/rckeqp.vssx"),
    },
    BuiltinStencil {
        slug: "server",
        category: "Network",
        title: "Servers",
        bytes: include_bytes!("../../assets/stencils/server.vssx"),
    },
    BuiltinStencil {
        slug: "spwa",
        category: "Software and Database",
        title: "Actions - SharePoint 2010 Workflow",
        bytes: include_bytes!("../../assets/stencils/spwa.vssx"),
    },
    BuiltinStencil {
        slug: "spwa2",
        category: "Software and Database",
        title: "Actions - SharePoint 2013 Workflow",
        bytes: include_bytes!("../../assets/stencils/spwa2.vssx"),
    },
    BuiltinStencil {
        slug: "dbchen",
        category: "Software and Database",
        title: "Chen’s Database Notation Shapes",
        bytes: include_bytes!("../../assets/stencils/dbchen.vssx"),
    },
    BuiltinStencil {
        slug: "comole",
        category: "Software and Database",
        title: "COM and OLE",
        bytes: include_bytes!("../../assets/stencils/comole.vssx"),
    },
    BuiltinStencil {
        slug: "wfcicn",
        category: "Software and Database",
        title: "Common Icons",
        bytes: include_bytes!("../../assets/stencils/wfcicn.vssx"),
    },
    BuiltinStencil {
        slug: "websit",
        category: "Software and Database",
        title: "Conceptual Web Site Shapes",
        bytes: include_bytes!("../../assets/stencils/websit.vssx"),
    },
    BuiltinStencil {
        slug: "spwc",
        category: "Software and Database",
        title: "Conditions - SharePoint 2010 Workflow",
        bytes: include_bytes!("../../assets/stencils/spwc.vssx"),
    },
    BuiltinStencil {
        slug: "spwc2",
        category: "Software and Database",
        title: "Conditions - SharePoint 2013 Workflow",
        bytes: include_bytes!("../../assets/stencils/spwc2.vssx"),
    },
    BuiltinStencil {
        slug: "spwt2",
        category: "Software and Database",
        title: "Containers & Terminators - SharePoint 2013 Workflow",
        bytes: include_bytes!("../../assets/stencils/spwt2.vssx"),
    },
    BuiltinStencil {
        slug: "wfctrl",
        category: "Software and Database",
        title: "Controls",
        bytes: include_bytes!("../../assets/stencils/wfctrl.vssx"),
    },
    BuiltinStencil {
        slug: "dbcrow",
        category: "Software and Database",
        title: "Crow’s Foot Database Notation Shapes",
        bytes: include_bytes!("../../assets/stencils/dbcrow.vssx"),
    },
    BuiltinStencil {
        slug: "wfcrs",
        category: "Software and Database",
        title: "Cursors",
        bytes: include_bytes!("../../assets/stencils/wfcrs.vssx"),
    },
    BuiltinStencil {
        slug: "datflo",
        category: "Software and Database",
        title: "Data Flow Diagram Shapes",
        bytes: include_bytes!("../../assets/stencils/datflo.vssx"),
    },
    BuiltinStencil {
        slug: "wfdeme",
        category: "Software and Database",
        title: "Department",
        bytes: include_bytes!("../../assets/stencils/wfdeme.vssx"),
    },
    BuiltinStencil {
        slug: "entapp",
        category: "Software and Database",
        title: "Enterprise Application",
        bytes: include_bytes!("../../assets/stencils/entapp.vssx"),
    },
    BuiltinStencil {
        slug: "wfdlgs",
        category: "Software and Database",
        title: "Forms and Dialogs",
        bytes: include_bytes!("../../assets/stencils/wfdlgs.vssx"),
    },
    BuiltinStencil {
        slug: "ganesa",
        category: "Software and Database",
        title: "Gane-Sarson",
        bytes: include_bytes!("../../assets/stencils/ganesa.vssx"),
    },
    BuiltinStencil {
        slug: "idef0",
        category: "Software and Database",
        title: "IDEF0 Diagram Shapes",
        bytes: include_bytes!("../../assets/stencils/idef0.vssx"),
    },
    BuiltinStencil {
        slug: "dbidef1x",
        category: "Software and Database",
        title: "IDEF1X Database Notation Shapes",
        bytes: include_bytes!("../../assets/stencils/dbidef1x.vssx"),
    },
    BuiltinStencil {
        slug: "langlv",
        category: "Software and Database",
        title: "Language Level Shapes",
        bytes: include_bytes!("../../assets/stencils/langlv.vssx"),
    },
    BuiltinStencil {
        slug: "memobj",
        category: "Software and Database",
        title: "Memory Objects",
        bytes: include_bytes!("../../assets/stencils/memobj.vssx"),
    },
    BuiltinStencil {
        slug: "spwt",
        category: "Software and Database",
        title: "Terminators - SharePoint 2010 Workflow",
        bytes: include_bytes!("../../assets/stencils/spwt.vssx"),
    },
    BuiltinStencil {
        slug: "wftlbr",
        category: "Software and Database",
        title: "Toolbars and Menus",
        bytes: include_bytes!("../../assets/stencils/wftlbr.vssx"),
    },
    BuiltinStencil {
        slug: "uactme",
        category: "Software and Database",
        title: "UML Activity Diagram Shapes",
        bytes: include_bytes!("../../assets/stencils/uactme.vssx"),
    },
    BuiltinStencil {
        slug: "ustrme",
        category: "Software and Database",
        title: "UML Class Diagram Shapes",
        bytes: include_bytes!("../../assets/stencils/ustrme.vssx"),
    },
    BuiltinStencil {
        slug: "dbuml",
        category: "Software and Database",
        title: "UML Database Notation",
        bytes: include_bytes!("../../assets/stencils/dbuml.vssx"),
    },
    BuiltinStencil {
        slug: "useqme",
        category: "Software and Database",
        title: "UML Sequence Diagram Shapes",
        bytes: include_bytes!("../../assets/stencils/useqme.vssx"),
    },
    BuiltinStencil {
        slug: "ustame",
        category: "Software and Database",
        title: "UML State Machine Shapes",
        bytes: include_bytes!("../../assets/stencils/ustame.vssx"),
    },
    BuiltinStencil {
        slug: "uuseme",
        category: "Software and Database",
        title: "UML Use Case Shapes",
        bytes: include_bytes!("../../assets/stencils/uuseme.vssx"),
    },
    BuiltinStencil {
        slug: "wfwicn",
        category: "Software and Database",
        title: "Web and Media Icons",
        bytes: include_bytes!("../../assets/stencils/wfwicn.vssx"),
    },
    BuiltinStencil {
        slug: "webmap",
        category: "Software and Database",
        title: "Web Site Map Shapes (US units)",
        bytes: include_bytes!("../../assets/stencils/webmap.vssx"),
    },
    BuiltinStencil {
        slug: "wfobme",
        category: "Software and Database",
        title: "Work Flow Objects",
        bytes: include_bytes!("../../assets/stencils/wfobme.vssx"),
    },
    BuiltinStencil {
        slug: "wfstme",
        category: "Software and Database",
        title: "Work Flow Steps",
        bytes: include_bytes!("../../assets/stencils/wfstme.vssx"),
    },
    BuiltinStencil {
        slug: "wfdep",
        category: "Software and Database",
        title: "Workflow Departments",
        bytes: include_bytes!("../../assets/stencils/wfdep.vssx"),
    },
    BuiltinStencil {
        slug: "wfobj",
        category: "Software and Database",
        title: "Workflow Objects",
        bytes: include_bytes!("../../assets/stencils/wfobj.vssx"),
    },
    BuiltinStencil {
        slug: "wfstep",
        category: "Software and Database",
        title: "Workflow Steps",
        bytes: include_bytes!("../../assets/stencils/wfstep.vssx"),
    },
    BuiltinStencil {
        slug: "alarm",
        category: "Maps and Floor Plans",
        title: "Alarm and Access Control",
        bytes: include_bytes!("../../assets/stencils/alarm.vssx"),
    },
    BuiltinStencil {
        slug: "appl",
        category: "Maps and Floor Plans",
        title: "Appliances",
        bytes: include_bytes!("../../assets/stencils/appl.vssx"),
    },
    BuiltinStencil {
        slug: "bthkt",
        category: "Maps and Floor Plans",
        title: "Bath And Kitchen Plan",
        bytes: include_bytes!("../../assets/stencils/bthkt.vssx"),
    },
    BuiltinStencil {
        slug: "bldcor",
        category: "Maps and Floor Plans",
        title: "Building Core",
        bytes: include_bytes!("../../assets/stencils/bldcor.vssx"),
    },
    BuiltinStencil {
        slug: "cabnt",
        category: "Maps and Floor Plans",
        title: "Cabinets",
        bytes: include_bytes!("../../assets/stencils/cabnt.vssx"),
    },
    BuiltinStencil {
        slug: "cubicl",
        category: "Maps and Floor Plans",
        title: "Cubicles",
        bytes: include_bytes!("../../assets/stencils/cubicl.vssx"),
    },
    BuiltinStencil {
        slug: "dimarc",
        category: "Maps and Floor Plans",
        title: "Dimensioning - Architectural",
        bytes: include_bytes!("../../assets/stencils/dimarc.vssx"),
    },
    BuiltinStencil {
        slug: "map3d",
        category: "Maps and Floor Plans",
        title: "Directional Map Shapes 3D",
        bytes: include_bytes!("../../assets/stencils/map3d.vssx"),
    },
    BuiltinStencil {
        slug: "eletel",
        category: "Maps and Floor Plans",
        title: "Electrical and Telecom",
        bytes: include_bytes!("../../assets/stencils/eletel.vssx"),
    },
    BuiltinStencil {
        slug: "furn",
        category: "Maps and Floor Plans",
        title: "Furniture",
        bytes: include_bytes!("../../assets/stencils/furn.vssx"),
    },
    BuiltinStencil {
        slug: "garden",
        category: "Maps and Floor Plans",
        title: "Garden Accessories",
        bytes: include_bytes!("../../assets/stencils/garden.vssx"),
    },
    BuiltinStencil {
        slug: "hvacc",
        category: "Maps and Floor Plans",
        title: "HVAC Controls",
        bytes: include_bytes!("../../assets/stencils/hvacc.vssx"),
    },
    BuiltinStencil {
        slug: "hvacce",
        category: "Maps and Floor Plans",
        title: "HVAC Controls Equipment",
        bytes: include_bytes!("../../assets/stencils/hvacce.vssx"),
    },
    BuiltinStencil {
        slug: "hvacd",
        category: "Maps and Floor Plans",
        title: "HVAC Ductwork",
        bytes: include_bytes!("../../assets/stencils/hvacd.vssx"),
    },
    BuiltinStencil {
        slug: "hvaceq",
        category: "Maps and Floor Plans",
        title: "HVAC Equipment",
        bytes: include_bytes!("../../assets/stencils/hvaceq.vssx"),
    },
    BuiltinStencil {
        slug: "irrig",
        category: "Maps and Floor Plans",
        title: "Irrigation",
        bytes: include_bytes!("../../assets/stencils/irrig.vssx"),
    },
    BuiltinStencil {
        slug: "lndmrk",
        category: "Maps and Floor Plans",
        title: "Landmark Shapes",
        bytes: include_bytes!("../../assets/stencils/lndmrk.vssx"),
    },
    BuiltinStencil {
        slug: "offacc",
        category: "Maps and Floor Plans",
        title: "Office Accessories",
        bytes: include_bytes!("../../assets/stencils/offacc.vssx"),
    },
    BuiltinStencil {
        slug: "offeqp",
        category: "Maps and Floor Plans",
        title: "Office Equipment",
        bytes: include_bytes!("../../assets/stencils/offeqp.vssx"),
    },
    BuiltinStencil {
        slug: "offfrn",
        category: "Maps and Floor Plans",
        title: "Office Furniture",
        bytes: include_bytes!("../../assets/stencils/offfrn.vssx"),
    },
    BuiltinStencil {
        slug: "prkrd",
        category: "Maps and Floor Plans",
        title: "Parking and Roads",
        bytes: include_bytes!("../../assets/stencils/prkrd.vssx"),
    },
    BuiltinStencil {
        slug: "pipe1",
        category: "Maps and Floor Plans",
        title: "Pipes and Valves - Pipes 1",
        bytes: include_bytes!("../../assets/stencils/pipe1.vssx"),
    },
    BuiltinStencil {
        slug: "pipe2",
        category: "Maps and Floor Plans",
        title: "Pipes and Valves - Pipes 2",
        bytes: include_bytes!("../../assets/stencils/pipe2.vssx"),
    },
    BuiltinStencil {
        slug: "valve1",
        category: "Maps and Floor Plans",
        title: "Pipes and Valves - Valves 1",
        bytes: include_bytes!("../../assets/stencils/valve1.vssx"),
    },
    BuiltinStencil {
        slug: "valve2",
        category: "Maps and Floor Plans",
        title: "Pipes and Valves - Valves 2",
        bytes: include_bytes!("../../assets/stencils/valve2.vssx"),
    },
    BuiltinStencil {
        slug: "plant",
        category: "Maps and Floor Plans",
        title: "Planting",
        bytes: include_bytes!("../../assets/stencils/plant.vssx"),
    },
    BuiltinStencil {
        slug: "plumb",
        category: "Maps and Floor Plans",
        title: "Plumbing",
        bytes: include_bytes!("../../assets/stencils/plumb.vssx"),
    },
    BuiltinStencil {
        slug: "ptsint",
        category: "Maps and Floor Plans",
        title: "Points Of Interest",
        bytes: include_bytes!("../../assets/stencils/ptsint.vssx"),
    },
    BuiltinStencil {
        slug: "recshp",
        category: "Maps and Floor Plans",
        title: "Recreation Shapes",
        bytes: include_bytes!("../../assets/stencils/recshp.vssx"),
    },
    BuiltinStencil {
        slug: "regstr",
        category: "Maps and Floor Plans",
        title: "Registers, Grilles and Diffusers",
        bytes: include_bytes!("../../assets/stencils/regstr.vssx"),
    },
    BuiltinStencil {
        slug: "road",
        category: "Maps and Floor Plans",
        title: "Road Shapes",
        bytes: include_bytes!("../../assets/stencils/road.vssx"),
    },
    BuiltinStencil {
        slug: "shpmch",
        category: "Maps and Floor Plans",
        title: "Shop Floor - Machines and Equipment",
        bytes: include_bytes!("../../assets/stencils/shpmch.vssx"),
    },
    BuiltinStencil {
        slug: "shpstr",
        category: "Maps and Floor Plans",
        title: "Shop Floor - Storage and Distribution",
        bytes: include_bytes!("../../assets/stencils/shpstr.vssx"),
    },
    BuiltinStencil {
        slug: "sitacc",
        category: "Maps and Floor Plans",
        title: "Site Accessories",
        bytes: include_bytes!("../../assets/stencils/sitacc.vssx"),
    },
    BuiltinStencil {
        slug: "recrt",
        category: "Maps and Floor Plans",
        title: "Sport Fields and Recreation",
        bytes: include_bytes!("../../assets/stencils/recrt.vssx"),
    },
    BuiltinStencil {
        slug: "transp",
        category: "Maps and Floor Plans",
        title: "Transportation Shapes",
        bytes: include_bytes!("../../assets/stencils/transp.vssx"),
    },
    BuiltinStencil {
        slug: "vehicl",
        category: "Maps and Floor Plans",
        title: "Vehicles",
        bytes: include_bytes!("../../assets/stencils/vehicl.vssx"),
    },
    BuiltinStencil {
        slug: "vidsur",
        category: "Maps and Floor Plans",
        title: "Video Surveillance",
        bytes: include_bytes!("../../assets/stencils/vidsur.vssx"),
    },
    BuiltinStencil {
        slug: "wadowi",
        category: "Maps and Floor Plans",
        title: "Walls, Doors and Windows",
        bytes: include_bytes!("../../assets/stencils/wadowi.vssx"),
    },
    BuiltinStencil {
        slug: "wall",
        category: "Maps and Floor Plans",
        title: "Walls, Shell and Structure",
        bytes: include_bytes!("../../assets/stencils/wall.vssx"),
    },
    BuiltinStencil {
        slug: "shprec",
        category: "Maps and Floor Plans",
        title: "Warehouse - Shipping and Receiving",
        bytes: include_bytes!("../../assets/stencils/shprec.vssx"),
    },
    BuiltinStencil {
        slug: "logic",
        category: "Engineering",
        title: "Analog and Digital Logic",
        bytes: include_bytes!("../../assets/stencils/logic.vssx"),
    },
    BuiltinStencil {
        slug: "eecomp",
        category: "Engineering",
        title: "Composite Assemblies",
        bytes: include_bytes!("../../assets/stencils/eecomp.vssx"),
    },
    BuiltinStencil {
        slug: "dimeng",
        category: "Engineering",
        title: "Dimensioning - Engineering",
        bytes: include_bytes!("../../assets/stencils/dimeng.vssx"),
    },
    BuiltinStencil {
        slug: "peeqp",
        category: "Engineering",
        title: "Equipment - General",
        bytes: include_bytes!("../../assets/stencils/peeqp.vssx"),
    },
    BuiltinStencil {
        slug: "peheat",
        category: "Engineering",
        title: "Equipment - Heat Exchangers",
        bytes: include_bytes!("../../assets/stencils/peheat.vssx"),
    },
    BuiltinStencil {
        slug: "pepump",
        category: "Engineering",
        title: "Equipment - Pumps",
        bytes: include_bytes!("../../assets/stencils/pepump.vssx"),
    },
    BuiltinStencil {
        slug: "pevess",
        category: "Engineering",
        title: "Equipment - Vessels",
        bytes: include_bytes!("../../assets/stencils/pevess.vssx"),
    },
    BuiltinStencil {
        slug: "fastn1",
        category: "Engineering",
        title: "Fasteners 1",
        bytes: include_bytes!("../../assets/stencils/fastn1.vssx"),
    },
    BuiltinStencil {
        slug: "fastn2",
        category: "Engineering",
        title: "Fasteners 2",
        bytes: include_bytes!("../../assets/stencils/fastn2.vssx"),
    },
    BuiltinStencil {
        slug: "fpeqp",
        category: "Engineering",
        title: "Fluid Power - Equipment",
        bytes: include_bytes!("../../assets/stencils/fpeqp.vssx"),
    },
    BuiltinStencil {
        slug: "fpassm",
        category: "Engineering",
        title: "Fluid Power - Valve Assembly",
        bytes: include_bytes!("../../assets/stencils/fpassm.vssx"),
    },
    BuiltinStencil {
        slug: "fpvalv",
        category: "Engineering",
        title: "Fluid Power - Valves",
        bytes: include_bytes!("../../assets/stencils/fpvalv.vssx"),
    },
    BuiltinStencil {
        slug: "eefund",
        category: "Engineering",
        title: "Fundamental Items",
        bytes: include_bytes!("../../assets/stencils/eefund.vssx"),
    },
    BuiltinStencil {
        slug: "gdt",
        category: "Engineering",
        title: "Geometric Dimensioning and Tolerancing",
        bytes: include_bytes!("../../assets/stencils/gdt.vssx"),
    },
    BuiltinStencil {
        slug: "intann",
        category: "Engineering",
        title: "Initiation and Annunciation",
        bytes: include_bytes!("../../assets/stencils/intann.vssx"),
    },
    BuiltinStencil {
        slug: "peinst",
        category: "Engineering",
        title: "Instruments",
        bytes: include_bytes!("../../assets/stencils/peinst.vssx"),
    },
    BuiltinStencil {
        slug: "eechip",
        category: "Engineering",
        title: "Integrated Circuit Components",
        bytes: include_bytes!("../../assets/stencils/eechip.vssx"),
    },
    BuiltinStencil {
        slug: "eemain",
        category: "Engineering",
        title: "Maintenance Symbols",
        bytes: include_bytes!("../../assets/stencils/eemain.vssx"),
    },
    BuiltinStencil {
        slug: "eemaps",
        category: "Engineering",
        title: "Maps and Charts",
        bytes: include_bytes!("../../assets/stencils/eemaps.vssx"),
    },
    BuiltinStencil {
        slug: "pepipe",
        category: "Engineering",
        title: "Pipelines",
        bytes: include_bytes!("../../assets/stencils/pepipe.vssx"),
    },
    BuiltinStencil {
        slug: "eequal",
        category: "Engineering",
        title: "Qualifying Symbols",
        bytes: include_bytes!("../../assets/stencils/eequal.vssx"),
    },
    BuiltinStencil {
        slug: "eemech",
        category: "Engineering",
        title: "Rotating Equip and Mech Functions",
        bytes: include_bytes!("../../assets/stencils/eemech.vssx"),
    },
    BuiltinStencil {
        slug: "eesemi",
        category: "Engineering",
        title: "Semiconductors And Electron Tubes",
        bytes: include_bytes!("../../assets/stencils/eesemi.vssx"),
    },
    BuiltinStencil {
        slug: "sprbr",
        category: "Engineering",
        title: "Springs and Bearings",
        bytes: include_bytes!("../../assets/stencils/sprbr.vssx"),
    },
    BuiltinStencil {
        slug: "eeswch",
        category: "Engineering",
        title: "Switches And Relays",
        bytes: include_bytes!("../../assets/stencils/eeswch.vssx"),
    },
    BuiltinStencil {
        slug: "eetcom",
        category: "Engineering",
        title: "Telecom Switch and Peripheral Equip",
        bytes: include_bytes!("../../assets/stencils/eetcom.vssx"),
    },
    BuiltinStencil {
        slug: "eeterm",
        category: "Engineering",
        title: "Terminals and Connectors",
        bytes: include_bytes!("../../assets/stencils/eeterm.vssx"),
    },
    BuiltinStencil {
        slug: "title",
        category: "Engineering",
        title: "Title Blocks",
        bytes: include_bytes!("../../assets/stencils/title.vssx"),
    },
    BuiltinStencil {
        slug: "eetran",
        category: "Engineering",
        title: "Transformers and Windings",
        bytes: include_bytes!("../../assets/stencils/eetran.vssx"),
    },
    BuiltinStencil {
        slug: "eepath",
        category: "Engineering",
        title: "Transmission Paths",
        bytes: include_bytes!("../../assets/stencils/eepath.vssx"),
    },
    BuiltinStencil {
        slug: "pevalv",
        category: "Engineering",
        title: "Valves and Fittings",
        bytes: include_bytes!("../../assets/stencils/pevalv.vssx"),
    },
    BuiltinStencil {
        slug: "eevhf",
        category: "Engineering",
        title: "VHF-UHF-SHF",
        bytes: include_bytes!("../../assets/stencils/eevhf.vssx"),
    },
    BuiltinStencil {
        slug: "weld",
        category: "Engineering",
        title: "Welding Symbols",
        bytes: include_bytes!("../../assets/stencils/weld.vssx"),
    },
    BuiltinStencil {
        slug: "calndr",
        category: "Schedule",
        title: "Calendar Shapes (US units)",
        bytes: include_bytes!("../../assets/stencils/calndr.vssx"),
    },
    BuiltinStencil {
        slug: "gantt",
        category: "Schedule",
        title: "Gantt Chart Shapes",
        bytes: include_bytes!("../../assets/stencils/gantt.vssx"),
    },
    BuiltinStencil {
        slug: "pert",
        category: "Schedule",
        title: "PERT Chart Shapes",
        bytes: include_bytes!("../../assets/stencils/pert.vssx"),
    },
    BuiltinStencil {
        slug: "timeln",
        category: "Schedule",
        title: "Timeline Shapes",
        bytes: include_bytes!("../../assets/stencils/timeln.vssx"),
    },
    BuiltinStencil {
        slug: "orgblt",
        category: "Organization Chart",
        title: "Belt - Organization Chart Shapes",
        bytes: include_bytes!("../../assets/stencils/orgblt.vssx"),
    },
    BuiltinStencil {
        slug: "orgbnd",
        category: "Organization Chart",
        title: "Bound - Organization Chart Shapes",
        bytes: include_bytes!("../../assets/stencils/orgbnd.vssx"),
    },
    BuiltinStencil {
        slug: "orgcns",
        category: "Organization Chart",
        title: "Coin - Organization Chart Shapes",
        bytes: include_bytes!("../../assets/stencils/orgcns.vssx"),
    },
    BuiltinStencil {
        slug: "orgnch",
        category: "Organization Chart",
        title: "Notch - Organization Chart Shapes",
        bytes: include_bytes!("../../assets/stencils/orgnch.vssx"),
    },
    BuiltinStencil {
        slug: "orgch",
        category: "Organization Chart",
        title: "Organization Chart Shapes",
        bytes: include_bytes!("../../assets/stencils/orgch.vssx"),
    },
    BuiltinStencil {
        slug: "orgpan",
        category: "Organization Chart",
        title: "Panel - Organization Chart Shapes",
        bytes: include_bytes!("../../assets/stencils/orgpan.vssx"),
    },
    BuiltinStencil {
        slug: "orgper",
        category: "Organization Chart",
        title: "Perspective - Organization Chart Shapes",
        bytes: include_bytes!("../../assets/stencils/orgper.vssx"),
    },
    BuiltinStencil {
        slug: "orgpet",
        category: "Organization Chart",
        title: "Petal - Organization Chart Shapes",
        bytes: include_bytes!("../../assets/stencils/orgpet.vssx"),
    },
    BuiltinStencil {
        slug: "orgpip",
        category: "Organization Chart",
        title: "Pip - Organization Chart Shapes",
        bytes: include_bytes!("../../assets/stencils/orgpip.vssx"),
    },
    BuiltinStencil {
        slug: "orgtac",
        category: "Organization Chart",
        title: "Shapetacular - Organization Chart Shapes",
        bytes: include_bytes!("../../assets/stencils/orgtac.vssx"),
    },
    BuiltinStencil {
        slug: "orgsto",
        category: "Organization Chart",
        title: "Stone - Organization Chart Shapes",
        bytes: include_bytes!("../../assets/stencils/orgsto.vssx"),
    },
    BuiltinStencil {
        slug: "bstorm",
        category: "Business",
        title: "Brainstorming",
        bytes: include_bytes!("../../assets/stencils/bstorm.vssx"),
    },
    BuiltinStencil {
        slug: "compln",
        category: "Business",
        title: "Compliance Shapes",
        bytes: include_bytes!("../../assets/stencils/compln.vssx"),
    },
    BuiltinStencil {
        slug: "itil",
        category: "Business",
        title: "ITIL Shapes",
        bytes: include_bytes!("../../assets/stencils/itil.vssx"),
    },
    BuiltinStencil {
        slug: "mrktdg",
        category: "Business",
        title: "Marketing Diagrams",
        bytes: include_bytes!("../../assets/stencils/mrktdg.vssx"),
    },
    BuiltinStencil {
        slug: "bpres",
        category: "Business",
        title: "Resources",
        bytes: include_bytes!("../../assets/stencils/bpres.vssx"),
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
    /// (#87): every catalog entry parses as a stencil, has visible
    /// masters, and its embedded bytes hash to the recorded digest.
    /// `digests.txt` is generated by `extract-stencils.rs`; editing an
    /// asset without re-running the extraction fails here.
    #[test]
    fn committed_assets_match_recorded_digests_and_parse() {
        let recorded: Vec<(String, String)> = include_str!("../../assets/stencils/digests.txt")
            .lines()
            .filter_map(|l| {
                let (hash, file) = l.split_once("  ")?;
                Some((hash.to_string(), file.to_string()))
            })
            .collect();
        assert_eq!(
            recorded.len(),
            BUILTINS.len(),
            "digest record covers every catalog entry"
        );
        for b in BUILTINS {
            let want = format!("{}.vssx", b.slug);
            let (hash, _) = recorded
                .iter()
                .find(|(_, f)| *f == want)
                .unwrap_or_else(|| panic!("{want} missing from digests.txt"));
            use sha2::{Digest, Sha256};
            let mut h = Sha256::new();
            h.update(b.bytes);
            let got: String = h.finalize().iter().map(|x| format!("{x:02x}")).collect();
            assert_eq!(&got, hash, "{} tampered or stale", want);

            let doc = vsdx::parse::parse_bytes(b.bytes)
                .unwrap_or_else(|e| panic!("{want} failed to parse: {e:#}"));
            assert!(!doc.masters.is_empty(), "{want} carries masters");
            assert!(doc.pages.is_empty(), "{want} is a stencil (no pages)");
        }
    }
}

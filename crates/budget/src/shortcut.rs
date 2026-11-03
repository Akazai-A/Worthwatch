//! The iPhone Shortcut, as a property list. The Wallet automation builds a dictionary
//! of the tap (merchant, amount, card, name) and runs this Shortcut with it; the Shortcut
//! stamps the time and either appends the tap to a file in iCloud Drive or, for
//! Tailscale, appends it to a queue file and posts the whole queue, deleting the queue
//! only when this server answers that it saved it. iOS never retries a failed Shortcut,
//! so the queue is written first and a tap made offline goes out with the next one.
//!
//! The output is unsigned. iOS imports only signed Shortcuts; on a Mac,
//! `shortcuts sign --mode anyone` signs it (the server does this from Settings).

use std::fmt::Write;

/// Text the server puts in a successful ingest reply; the Shortcut looks for it before
/// deleting its queue. A 401 or an error page never contains it.
pub const SAVED_MARKER: &str = "worthwatch_saved";
/// Folder inside the Shortcuts iCloud folder.
pub const FOLDER: &str = "Worthwatch";
pub const QUEUE: &str = "Worthwatch/queue.jsonl";
/// Written by the Mac for the iCloud Shortcuts: what's left today, the morning recap.
pub const STATUS: &str = "Worthwatch/game/today.json";

pub enum Transport<'a> {
    Icloud,
    Tailscale { url: &'a str, token: &'a str },
}

enum P {
    S(String),
    I(i64),
    B(bool),
    A(Vec<P>),
    D(Vec<(&'static str, P)>),
    /// A dictionary with keys that are not static (attachmentsByRange).
    Dk(Vec<(String, P)>),
}

fn s(v: &str) -> P {
    P::S(v.to_string())
}

fn esc(v: &str) -> String {
    v.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn write(p: &P, out: &mut String) {
    match p {
        P::S(v) => {
            let _ = write!(out, "<string>{}</string>", esc(v));
        }
        P::I(v) => {
            let _ = write!(out, "<integer>{v}</integer>");
        }
        P::B(v) => out.push_str(if *v { "<true/>" } else { "<false/>" }),
        P::A(items) => {
            out.push_str("<array>");
            for i in items {
                write(i, out);
            }
            out.push_str("</array>");
        }
        P::D(kv) => {
            out.push_str("<dict>");
            for (k, v) in kv {
                let _ = write!(out, "<key>{}</key>", esc(k));
                write(v, out);
            }
            out.push_str("</dict>");
        }
        P::Dk(kv) => {
            out.push_str("<dict>");
            for (k, v) in kv {
                let _ = write!(out, "<key>{}</key>", esc(k));
                write(v, out);
            }
            out.push_str("</dict>");
        }
    }
}

fn output(uuid: &str, name: &str) -> P {
    P::D(vec![("Type", s("ActionOutput")), ("OutputUUID", s(uuid)), ("OutputName", s(name))])
}

/// A single variable as a parameter value.
fn attachment(value: P) -> P {
    P::D(vec![("Value", value), ("WFSerializationType", s("WFTextTokenAttachment"))])
}

/// Text with variables: `parts` alternate literal text and variables.
fn text(parts: Vec<Result<&str, P>>) -> P {
    let mut string = String::new();
    let mut ranges = Vec::new();
    for part in parts {
        match part {
            Ok(t) => string.push_str(t),
            Err(var) => {
                // Ranges count UTF-16 units; everything here is ASCII but the placeholder.
                let at = string.encode_utf16().count();
                ranges.push((format!("{{{at}, 1}}"), var));
                string.push('\u{fffc}');
            }
        }
    }
    P::D(vec![
        ("Value", P::D(vec![("string", P::S(string)), ("attachmentsByRange", P::Dk(ranges))])),
        ("WFSerializationType", s("WFTextTokenString")),
    ])
}

fn action(id: &str, params: Vec<(&'static str, P)>) -> P {
    P::D(vec![("WFWorkflowActionIdentifier", s(id)), ("WFWorkflowActionParameters", P::D(params))])
}

fn current_date() -> P {
    P::D(vec![("Type", s("CurrentDate"))])
}

const U_TIME: &str = "9B1C2D3E-0001-4A5B-8C7D-000000000001";
const U_MONTH: &str = "9B1C2D3E-0002-4A5B-8C7D-000000000002";
const U_DICT: &str = "9B1C2D3E-0003-4A5B-8C7D-000000000003";
const U_TAP: &str = "9B1C2D3E-0004-4A5B-8C7D-000000000004";
const U_FILE: &str = "9B1C2D3E-0006-4A5B-8C7D-000000000006";
const U_POST: &str = "9B1C2D3E-0007-4A5B-8C7D-000000000007";
const G_IF: &str = "9B1C2D3E-0008-4A5B-8C7D-000000000008";
const U_STATUS_FILE: &str = "9B1C2D3E-0009-4A5B-8C7D-000000000009";
const U_STATUS: &str = "9B1C2D3E-000A-4A5B-8C7D-00000000000A";
const U_LINE: &str = "9B1C2D3E-000B-4A5B-8C7D-00000000000B";
const U_MERCHANT: &str = "9B1C2D3E-000C-4A5B-8C7D-00000000000C";
const U_AMOUNT: &str = "9B1C2D3E-000D-4A5B-8C7D-00000000000D";
const G_NOTIFY: &str = "9B1C2D3E-000E-4A5B-8C7D-00000000000E";

fn value_for_key(uuid: &str, dict: P, key: &str) -> P {
    action(
        "is.workflow.actions.getvalueforkey",
        vec![("UUID", s(uuid)), ("WFInput", attachment(dict)), ("WFDictionaryKey", s(key))],
    )
}

/// `If <line> has any value: Show Notification(title, line) End If`.
fn notify_if_any(title: Vec<Result<&str, P>>, line: &str, group: &str) -> Vec<P> {
    vec![
        action(
            "is.workflow.actions.conditional",
            vec![
                ("GroupingIdentifier", s(group)),
                ("WFControlFlowMode", P::I(0)),
                ("WFCondition", P::I(100)),
                (
                    "WFInput",
                    P::D(vec![
                        ("Type", s("Variable")),
                        ("Variable", attachment(output(line, "Dictionary Value"))),
                    ]),
                ),
            ],
        ),
        action(
            "is.workflow.actions.notification",
            vec![
                ("WFNotificationActionTitle", text(title)),
                ("WFNotificationActionBody", text(vec![Err(output(line, "Dictionary Value"))])),
                ("WFNotificationActionSound", P::B(false)),
            ],
        ),
        action(
            "is.workflow.actions.conditional",
            vec![("GroupingIdentifier", s(group)), ("WFControlFlowMode", P::I(2))],
        ),
    ]
}

/// Read `Worthwatch/game/today.json` and take one key out of it.
fn read_status(key: &str) -> Vec<P> {
    vec![
        action(
            "is.workflow.actions.documentpicker.open",
            vec![
                ("UUID", s(U_STATUS_FILE)),
                ("WFGetFilePath", text(vec![Ok(STATUS)])),
                ("WFFileErrorIfNotFound", P::B(false)),
                ("WFShowFilePicker", P::B(false)),
            ],
        ),
        action(
            "is.workflow.actions.detect.dictionary",
            vec![("UUID", s(U_STATUS)), ("WFInput", attachment(output(U_STATUS_FILE, "File")))],
        ),
        value_for_key(U_LINE, output(U_STATUS, "Dictionary"), key),
    ]
}

fn bearer(token: &str) -> P {
    let header = P::D(vec![
        ("WFItemType", P::I(0)),
        ("WFKey", text(vec![Ok("Authorization")])),
        ("WFValue", text(vec![Ok(&format!("Bearer {token}"))])),
    ]);
    P::D(vec![
        ("Value", P::D(vec![("WFDictionaryFieldValueItems", P::A(vec![header]))])),
        ("WFSerializationType", s("WFDictionaryFieldValue")),
    ])
}

/// The Shortcut's property list (XML). Keep the token out of logs: it is in here.
pub fn plist(transport: Transport) -> String {
    let mut actions = vec![
        action(
            "is.workflow.actions.format.date",
            vec![
                ("UUID", s(U_TIME)),
                ("WFDateFormatStyle", s("Custom")),
                ("WFDateFormat", s("yyyy-MM-dd'T'HH:mm:ss.SSSZZZZZ")),
                ("WFDate", text(vec![Err(current_date())])),
            ],
        ),
        action(
            "is.workflow.actions.format.date",
            vec![
                ("UUID", s(U_MONTH)),
                ("WFDateFormatStyle", s("Custom")),
                ("WFDateFormat", s("yyyy-MM")),
                ("WFDate", text(vec![Err(current_date())])),
            ],
        ),
        action(
            "is.workflow.actions.detect.dictionary",
            vec![("UUID", s(U_DICT)), ("WFInput", attachment(P::D(vec![("Type", s("ExtensionInput"))])))],
        ),
        action(
            "is.workflow.actions.setvalueforkey",
            vec![
                ("UUID", s(U_TAP)),
                ("WFDictionary", attachment(output(U_DICT, "Dictionary"))),
                ("WFDictionaryKey", s("at")),
                ("WFDictionaryValue", text(vec![Err(output(U_TIME, "Formatted Date"))])),
            ],
        ),
    ];
    // The notification's title: "Pai Northern Thai · $24.00", from the tap itself.
    actions.push(value_for_key(U_MERCHANT, output(U_DICT, "Dictionary"), "merchant"));
    actions.push(value_for_key(U_AMOUNT, output(U_DICT, "Dictionary"), "amount"));
    let title = || {
        vec![
            Err(output(U_MERCHANT, "Dictionary Value")),
            Ok(" · "),
            Err(output(U_AMOUNT, "Dictionary Value")),
        ]
    };
    let append = |path: P| {
        action(
            "is.workflow.actions.file.append",
            vec![
                ("WFFilePath", path),
                ("WFInput", attachment(output(U_TAP, "Dictionary"))),
                ("WFAppendOnNewLine", P::B(true)),
            ],
        )
    };
    match transport {
        Transport::Icloud => {
            actions.push(append(text(vec![
                Ok("Worthwatch/taps-"),
                Err(output(U_MONTH, "Formatted Date")),
                Ok(".jsonl"),
            ])));
            // No round trip in iCloud mode: show what the Mac last wrote.
            actions.extend(read_status("tap"));
            actions.extend(notify_if_any(title(), U_LINE, G_NOTIFY));
        }
        Transport::Tailscale { url, token } => {
            let endpoint = format!("{}/api/ingest/transaction", url.trim_end_matches('/'));
            actions.push(append(text(vec![Ok(QUEUE)])));
            actions.push(action(
                "is.workflow.actions.documentpicker.open",
                vec![
                    ("UUID", s(U_FILE)),
                    ("WFGetFilePath", text(vec![Ok(QUEUE)])),
                    ("WFFileErrorIfNotFound", P::B(false)),
                    ("WFShowFilePicker", P::B(false)),
                ],
            ));
            actions.push(action(
                "is.workflow.actions.downloadurl",
                vec![
                    ("UUID", s(U_POST)),
                    ("WFURL", s(&endpoint)),
                    ("WFHTTPMethod", s("POST")),
                    ("WFHTTPBodyType", s("File")),
                    ("WFRequestVariable", attachment(output(U_FILE, "File"))),
                    ("ShowHeaders", P::B(true)),
                    ("WFHTTPHeaders", bearer(token)),
                ],
            ));
            actions.push(action(
                "is.workflow.actions.conditional",
                vec![
                    ("GroupingIdentifier", s(G_IF)),
                    ("WFControlFlowMode", P::I(0)),
                    ("WFCondition", P::I(99)),
                    (
                        "WFInput",
                        P::D(vec![
                            ("Type", s("Variable")),
                            ("Variable", attachment(output(U_POST, "Contents of URL"))),
                        ]),
                    ),
                    ("WFConditionalActionString", s(SAVED_MARKER)),
                ],
            ));
            actions.push(action(
                "is.workflow.actions.file.delete",
                vec![
                    ("WFInput", attachment(output(U_FILE, "File"))),
                    ("WFDeleteFileConfirmDeletion", P::B(false)),
                ],
            ));
            actions.push(action(
                "is.workflow.actions.conditional",
                vec![("GroupingIdentifier", s(G_IF)), ("WFControlFlowMode", P::I(2))],
            ));
            actions.push(value_for_key(U_LINE, output(U_POST, "Contents of URL"), "notify"));
            actions.extend(notify_if_any(title(), U_LINE, G_NOTIFY));
        }
    }
    wrap(actions, true)
}

/// The morning recap Shortcut ("Worthwatch Day"), run by a time-of-day automation:
/// how yesterday closed and today's allowance, as a notification.
pub fn day_plist(transport: Transport) -> String {
    let mut actions = Vec::new();
    match transport {
        Transport::Icloud => actions.extend(read_status("day")),
        Transport::Tailscale { url, token } => {
            let endpoint = format!("{}/api/ingest/day", url.trim_end_matches('/'));
            actions.push(action(
                "is.workflow.actions.downloadurl",
                vec![
                    ("UUID", s(U_POST)),
                    ("WFURL", s(&endpoint)),
                    ("WFHTTPMethod", s("GET")),
                    ("ShowHeaders", P::B(true)),
                    ("WFHTTPHeaders", bearer(token)),
                ],
            ));
            actions.push(value_for_key(U_LINE, output(U_POST, "Contents of URL"), "text"));
        }
    }
    actions.extend(notify_if_any(vec![Ok("Worthwatch")], U_LINE, G_NOTIFY));
    wrap(actions, false)
}

fn wrap(actions: Vec<P>, takes_input: bool) -> String {
    let root = P::D(vec![
        ("WFWorkflowClientVersion", s("2302.0.4")),
        ("WFWorkflowMinimumClientVersion", P::I(900)),
        ("WFWorkflowMinimumClientVersionString", s("900")),
        (
            "WFWorkflowIcon",
            P::D(vec![
                ("WFWorkflowIconGlyphNumber", P::I(59511)),
                ("WFWorkflowIconStartColor", P::I(4_282_601_983)),
            ]),
        ),
        (
            "WFWorkflowInputContentItemClasses",
            P::A(if takes_input {
                vec![s("WFDictionaryContentItem"), s("WFStringContentItem")]
            } else {
                vec![]
            }),
        ),
        ("WFWorkflowHasShortcutInputVariables", P::B(takes_input)),
        ("WFWorkflowTypes", P::A(vec![])),
        ("WFWorkflowImportQuestions", P::A(vec![])),
        ("WFWorkflowActions", P::A(actions)),
    ]);
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">",
    );
    write(&root, &mut out);
    out.push_str("</plist>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icloud_appends_to_a_monthly_file() {
        let p = plist(Transport::Icloud);
        assert!(p.starts_with("<?xml"));
        assert!(p.contains("is.workflow.actions.file.append"));
        assert!(p.contains("Worthwatch/taps-\u{fffc}.jsonl"));
        assert!(!p.contains("downloadurl"));
        assert_eq!(p.matches("<dict>").count(), p.matches("</dict>").count());
        // Then reads the Mac's status file and shows its line.
        let order = ["file.append", STATUS, "<string>tap</string>", "actions.notification"];
        let at: Vec<usize> = order.iter().map(|a| p.find(a).unwrap()).collect();
        assert!(at.windows(2).all(|w| w[0] < w[1]), "append, read status, notify");
    }

    #[test]
    fn day_shortcut_reads_the_recap_and_takes_no_input() {
        let p = day_plist(Transport::Icloud);
        assert!(
            p.contains(STATUS) && p.contains("<string>day</string>") && p.contains("actions.notification")
        );
        assert!(p.contains("<key>WFWorkflowHasShortcutInputVariables</key><false/>"));
        let p = day_plist(Transport::Tailscale { url: "https://m.ts.net", token: "abc" });
        assert!(p.contains("https://m.ts.net/api/ingest/day") && p.contains("Bearer abc"));
        assert!(p.contains("<string>text</string>"));
        assert_eq!(p.matches("<dict>").count(), p.matches("</dict>").count());
    }

    #[test]
    fn tailscale_queues_posts_and_clears_only_on_success() {
        let p = plist(Transport::Tailscale { url: "https://mac-mini.tail1234.ts.net/", token: "t0k&n" });
        assert!(p.contains("https://mac-mini.tail1234.ts.net/api/ingest/transaction"));
        assert!(p.contains("Bearer t0k&amp;n"), "escaped in XML");
        let order = ["file.append", "documentpicker.open", "downloadurl", "conditional", "file.delete"];
        let at: Vec<usize> = order.iter().map(|a| p.find(a).unwrap()).collect();
        assert!(at.windows(2).all(|w| w[0] < w[1]), "queue first, delete last");
        assert!(p.contains(SAVED_MARKER));
        assert!(p.find("<string>notify</string>").unwrap() > p.find("file.delete").unwrap());
        assert!(p.contains("is.workflow.actions.notification"));
        assert_eq!(p.matches("<array>").count(), p.matches("</array>").count());
    }
}

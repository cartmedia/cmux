// l10n-allow-file: scratch gallery fixtures for the cards/default look before shots (never committed).
// Today's Cards and Default New Tab templates at base, with no chats, a few, and 10,000.
import { agentPaneEntry } from "../../../gallery/format";
import { CWD, manySessions, noChat, session } from "../../../gallery/fixtures/acpmux";
import { minutesAgo } from "../../../gallery/clock";

const PROJECTS = ["/Users/you/src/atlas-web", "/Users/you/src/cmux", "/Users/you/src/manaflow", "/Users/you/src/notes"];

const newTab = (fields: Record<string, unknown> = {}) => ({
  newTab: { layout: "b", kind: "agent", cwd: CWD, home: "/Users/you", lastAgent: "claude", projects: PROJECTS, ...fields },
  newSession: true,
});

/// Chats where three need the user, then quiet ones (the Default template's suggestions card).
const attention = () => [
  session({
    sessionId: "look-failed",
    title: "Composer + picker polish",
    indicator: "failed",
    preview: "Hit an error while running the gallery checks.",
    cwd: "/Users/you/src/cmux",
    updatedAt: minutesAgo(2),
  }),
  session({
    sessionId: "look-input",
    title: "Ship the release notes for 0.64",
    status: "waiting",
    pendingPermissions: 1,
    indicator: "input",
    cwd: "/Users/you/src/manaflow",
    updatedAt: minutesAgo(12),
  }),
  ...manySessions(6).map((entry) => ({ ...entry, indicator: "done" as const, status: "idle" as const, pendingPermissions: 0 })),
];

const many = () =>
  manySessions(10000).map((entry, index) => ({ ...entry, sessionId: `big-${index}` }));

export default agentPaneEntry({
  id: "agent-pane.new-tab-looks-before",
  title: "New Tab looks: before",
  area: "New Tab",
  height: 720,
  covers: ["agent-session/acpmux/newtab/screen.css"],
  variants: {
    "cards-empty": { note: "Cards, no chats.", ready: newTab({ template: "cards" }), snapshot: noChat([]) },
    "cards-few": { note: "Cards, six chats.", ready: newTab({ template: "cards" }), snapshot: noChat(manySessions(6)) },
    "cards-10k": { note: "Cards, 10,000 chats.", ready: newTab({ template: "cards" }), snapshot: noChat(many()) },
    "default-empty": { note: "Default, no chats.", ready: newTab({ template: "default" }), snapshot: noChat([]) },
    "default-few": { note: "Default, chats that need you and quiet ones.", ready: newTab({ template: "default" }), snapshot: noChat(attention()) },
    "default-10k": { note: "Default, 10,000 chats.", ready: newTab({ template: "default" }), snapshot: noChat(many()) },
  },
});

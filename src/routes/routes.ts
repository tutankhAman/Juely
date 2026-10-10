// Recovered route table from Pluely 1.1.0.
export interface AppRoute { path: string; component: string }

export const ROUTES: AppRoute[] = [
  { path: "/", component: "brt" },
  { path: "/dashboard", component: "VQe" },
  { path: "/chats", component: "Ztt" },
  { path: "/system-prompts", component: "krt" },
  { path: "/files", component: "yat" },
  { path: "/chats/view/:conversationId", component: "ent" },
  { path: "/meetings/view/:meetingId", component: "nnt" },
  { path: "/shortcuts", component: "Zrt" },
  { path: "/screenshot", component: "oat" },
  { path: "/capture", component: "cat" },
  { path: "/settings", component: "Drt" },
  { path: "/audio", component: "iat" },
  { path: "/responses", component: "gat" },
  { path: "/dev-space", component: "Vrt" },
];

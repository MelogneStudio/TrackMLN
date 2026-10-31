import { getCurrentWindow } from "@tauri-apps/api/window";
import { AnnoyWindow } from "./components/AnnoyWindow";
import { WarnWindow } from "./components/WarnWindow";
import { PrefsProvider } from "./lib/prefs";
import { Overlay } from "./overlay/Overlay";
import { AppShell } from "./shell/AppShell";

const label = getCurrentWindow().label;

function Root() {
  switch (label) {
    case "warn":
      return <WarnWindow />;
    case "annoy":
      return <AnnoyWindow />;
    case "overlay":
      return <Overlay />;
    default:
      return <AppShell />;
  }
}

export default function App() {
  return (
    <PrefsProvider>
      <Root />
    </PrefsProvider>
  );
}

let deferredInstall: BeforeInstallPromptEvent | null = null;

export function captureInstallPrompt(event: Event) {
  event.preventDefault();
  deferredInstall = event as BeforeInstallPromptEvent;
}

export const webTools = {
  async fullscreen(): Promise<void> { if (!document.fullscreenElement) await document.documentElement.requestFullscreen(); else await document.exitFullscreen(); },
  async notifications(): Promise<NotificationPermission> { if (!("Notification" in window)) return "denied"; return Notification.requestPermission(); },
  canInstall: () => deferredInstall !== null,
  async install(): Promise<boolean> { if (!deferredInstall) return false; await deferredInstall.prompt(); const result = await deferredInstall.userChoice; deferredInstall = null; return result.outcome === "accepted"; },
};

export interface BeforeInstallPromptEvent extends Event {
  prompt: () => Promise<void>;
  userChoice: Promise<{ outcome: "accepted" | "dismissed" }>;
}

export interface Settings {
  launch_at_startup: boolean;
  minimize_to_tray: boolean;
  hardware_acceleration: boolean;
  pause_on_battery: boolean;
  pause_when_maximized: boolean;
  scaling_mode: string;
  accent_color: "violet" | "cyan" | "rose" | "emerald";
  ui_scale: number;
}

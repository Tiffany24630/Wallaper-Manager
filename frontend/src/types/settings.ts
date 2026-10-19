export interface Settings {
  launch_at_startup: boolean;
  minimize_to_tray: boolean;
  hardware_acceleration: boolean;
  pause_on_battery: boolean;
  pause_when_maximized: boolean;
  scaling_mode: string;
  accent_color: "violet" | "cyan" | "rose" | "emerald";
  ui_scale: number;
  dynamic_enabled: boolean;
  dynamic_playlist_id: string | null;
  dynamic_interval_minutes: number;
  dynamic_current_wallpaper_id: string | null;
  dynamic_last_error: string | null;
  cursor_enabled: boolean;
  active_cursor_id: string | null;
}

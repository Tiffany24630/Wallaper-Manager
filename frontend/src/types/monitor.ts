export interface Monitor {
  id: string;
  name: string;
  width: number;
  height: number;
  refreshRate: number;
  primary: boolean;
}

export interface MonitorPlaylist {
  monitorId: string;
  playlistId: string;
  currentWallpaperId?: string | null;
  lastError?: string | null;
}

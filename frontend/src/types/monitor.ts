export interface Monitor {
  id: string;
  name: string;
  width: number;
  height: number;
  refreshRate: number;
  primary: boolean;
  wallpaperId?: string;
}
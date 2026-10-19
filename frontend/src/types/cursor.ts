export interface CursorResource {
  id: string;
  name: string;
  path: string;
  preview?: string;
  resourceType: "static" | "animated";
  sourcePath?: string;
  hotspotX: number;
  hotspotY: number;
  createdAt?: string;
}

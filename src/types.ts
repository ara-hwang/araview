export type ImageInfo = {
  base64: string;
  mime_type: string;
  file_name: string;
  file_size: number;
};

export type DirectoryImages = {
  images: string[];
  current_index: number;
};

export type BackgroundType = "checkered" | "solid";

export type CacheMode = "off" | "nearby" | "extended";

export type Settings = {
  background: BackgroundType;
  loopNavigation: boolean;
  cacheMode: CacheMode;
};

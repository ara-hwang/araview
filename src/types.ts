export interface ImageInfo {
  base64: string;
  mime_type: string;
  file_name: string;
  file_size: number;
}

export interface DirectoryImages {
  images: string[];
  current_index: number;
}

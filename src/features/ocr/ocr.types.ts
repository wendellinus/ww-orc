export type OcrPoint = {
  x: number;
  y: number;
};

export type OcrTextBlock = {
  text: string;
  boxPoints: OcrPoint[];
};

export type OcrResult = {
  imageId: string;
  workspaceId: string;
  fileName: string;
  imagePath: string;
  status: "unrecognized" | "pending" | "completed" | "failed";
  text: string;
  blocks: OcrTextBlock[];
  errorMessage: string | null;
  createdAt: number;
};

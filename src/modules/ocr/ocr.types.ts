export type OcrResult = {
  imageId: string;
  workspaceId: string;
  fileName: string;
  imagePath: string;
  status: "pending" | "completed" | "failed";
  text: string;
  errorMessage: string | null;
  createdAt: number;
};

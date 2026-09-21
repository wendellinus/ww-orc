export type OcrResult = {
  imageId: string;
  workspaceId: string;
  fileName: string;
  imagePath: string;
  status: "unrecognized" | "pending" | "completed" | "failed";
  text: string;
  errorMessage: string | null;
  createdAt: number;
};

// ============================================================================
// Data Structures (DTOs from storage layer)
// ============================================================================

export type ReportDocument = {
  id: string;
  companyId: string;
  periodId: string | null;
  sourceType: string;
  originRef: string | null;
  url: string;
  localPath: string | null;
  contentType: string | null;
  contentHash: string | null;
  byteSize: number | null;
  title: string | null;
  attribution: string | null;
  fetchStatus: string;
  fetchError: string | null;
  fetchedAt: string | null;
  createdAt: string;
  updatedAt: string;
};

export type CaptureReportDocumentInput = {
  companyId: string;
  sourceType: string;
  url: string;
  periodId?: string;
  originRef?: string;
  title?: string;
  attribution?: string;
};

export type DocumentCaptureResult = {
  documentId: string;
  localPath: string | null;
  success: boolean;
  error: string | null;
};

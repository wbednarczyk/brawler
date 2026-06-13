import { invoke } from "@tauri-apps/api/core";
import {
  ReportDocument,
  CaptureReportDocumentInput,
  DocumentCaptureResult,
} from "./reportDocumentsTypes";

export async function captureReportDocument(
  input: CaptureReportDocumentInput
): Promise<DocumentCaptureResult> {
  return invoke("capture_report_document", { input });
}

export async function listReportDocuments(
  companyId: string
): Promise<ReportDocument[]> {
  return invoke("list_report_documents", { companyId });
}

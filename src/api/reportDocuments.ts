import { invoke } from "@tauri-apps/api/core";
import {
  ReportDocument,
  CaptureReportDocumentInput,
  DocumentCaptureResult,
  ReclassifyReportDocumentsSummary,
  ReportDocumentsView,
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

export async function reclassifyReportDocuments(): Promise<ReclassifyReportDocumentsSummary> {
  return invoke("reclassify_report_documents");
}

export async function getReportDocumentsView(
  companyId: string
): Promise<ReportDocumentsView> {
  return invoke("get_report_documents_view", { companyId });
}

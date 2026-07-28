import { invoke } from "@tauri-apps/api/core";
import {
  ReclassifyReportDocumentsSummary,
  ReportDocumentsView,
} from "./reportDocumentsTypes";

export async function reclassifyReportDocuments(): Promise<ReclassifyReportDocumentsSummary> {
  return invoke("reclassify_report_documents");
}

export async function getReportDocumentsView(
  companyId: string
): Promise<ReportDocumentsView> {
  return invoke("get_report_documents_view", { companyId });
}

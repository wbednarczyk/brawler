export type NotebookForm = {
  title: string;
  body: string;
  tags: string;
  kind: string;
  claimStatus: string;
  eventDate: string;
  followUpAfter: string;
  followUpDate: string;
};

export type NotebookDateLikeFieldProps = {
  label: string;
  ariaLabel: string;
  value: string;
  onChange: (value: string) => void;
};

export type MarkdownNoteBodyProps = {
  body: string;
  ariaLabel?: string;
};

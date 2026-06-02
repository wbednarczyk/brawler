import type { Dispatch, KeyboardEvent, MutableRefObject, PointerEvent, SetStateAction } from "react";
import { detailPaneMaxWidth, detailPaneMinWidth } from "./layout";

type DetailPaneResizeInput = {
  contentGridRef: MutableRefObject<HTMLElement | null>;
  setDetailPaneWidth: Dispatch<SetStateAction<number>>;
};

export function useDetailPaneResize({
  contentGridRef,
  setDetailPaneWidth,
}: DetailPaneResizeInput) {
  function clampDetailPaneWidth(width: number) {
    const gridWidth = contentGridRef.current?.getBoundingClientRect().width ?? 0;
    const responsiveMaxWidth = gridWidth > 0 ? Math.min(detailPaneMaxWidth, gridWidth * 0.55) : detailPaneMaxWidth;

    return Math.round(Math.min(Math.max(width, detailPaneMinWidth), responsiveMaxWidth));
  }

  function resizeDetailPaneFromPointer(clientX: number) {
    const gridBounds = contentGridRef.current?.getBoundingClientRect();

    if (!gridBounds) {
      return;
    }

    setDetailPaneWidth(clampDetailPaneWidth(gridBounds.right - clientX));
  }

  function startDetailPaneResize(event: PointerEvent<HTMLDivElement>) {
    event.currentTarget.setPointerCapture(event.pointerId);
    resizeDetailPaneFromPointer(event.clientX);
  }

  function resizeDetailPane(event: PointerEvent<HTMLDivElement>) {
    if (!event.currentTarget.hasPointerCapture(event.pointerId)) {
      return;
    }

    resizeDetailPaneFromPointer(event.clientX);
  }

  function stopDetailPaneResize(event: PointerEvent<HTMLDivElement>) {
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
  }

  function resizeDetailPaneWithKeyboard(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key === "ArrowLeft") {
      event.preventDefault();
      setDetailPaneWidth((current) => clampDetailPaneWidth(current + 24));
    }

    if (event.key === "ArrowRight") {
      event.preventDefault();
      setDetailPaneWidth((current) => clampDetailPaneWidth(current - 24));
    }
  }

  return {
    resizeDetailPane,
    resizeDetailPaneWithKeyboard,
    startDetailPaneResize,
    stopDetailPaneResize,
  };
}

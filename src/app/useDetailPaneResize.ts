import type { Dispatch, KeyboardEvent, MutableRefObject, PointerEvent, SetStateAction } from "react";
import { detailPaneMaxFraction, detailPaneMinFraction } from "./layout";

const keyboardStep = 0.03;

type DetailPaneResizeInput = {
  contentGridRef: MutableRefObject<HTMLElement | null>;
  setDetailPaneFraction: Dispatch<SetStateAction<number>>;
};

export function useDetailPaneResize({
  contentGridRef,
  setDetailPaneFraction,
}: DetailPaneResizeInput) {
  function clampDetailPaneFraction(fraction: number) {
    return Math.min(detailPaneMaxFraction, Math.max(detailPaneMinFraction, fraction));
  }

  function resizeDetailPaneFromPointer(clientX: number) {
    const gridBounds = contentGridRef.current?.getBoundingClientRect();

    if (!gridBounds || gridBounds.width <= 0) {
      return;
    }

    // The detail pane is the right region of the grid, so its size is the
    // distance from the pointer across to the grid's right edge.
    setDetailPaneFraction(clampDetailPaneFraction((gridBounds.right - clientX) / gridBounds.width));
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
    // The divider is vertical: ArrowLeft moves it left, growing the right detail
    // pane; ArrowRight shrinks it.
    if (event.key === "ArrowLeft") {
      event.preventDefault();
      setDetailPaneFraction((current) => clampDetailPaneFraction(current + keyboardStep));
    }

    if (event.key === "ArrowRight") {
      event.preventDefault();
      setDetailPaneFraction((current) => clampDetailPaneFraction(current - keyboardStep));
    }
  }

  return {
    resizeDetailPane,
    resizeDetailPaneWithKeyboard,
    startDetailPaneResize,
    stopDetailPaneResize,
  };
}

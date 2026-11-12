import React, { useEffect, useRef } from "react";
import { EditorState } from "@codemirror/state";
import { EditorView, keymap, lineNumbers } from "@codemirror/view";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";

interface LatexEditorProps {
  value: string;
  onChange: (next: string) => void;
}

const editorTheme = EditorView.theme({
  "&": {
    height: "100%",
    fontSize: "13px",
    lineHeight: "1.6",
    fontFamily: "'SF Mono', ui-monospace, SFMono-Regular, Menlo, monospace",
    backgroundColor: "#050506",
    color: "#f5f5f7",
  },
  ".cm-content": {
    padding: "16px 8px",
    caretColor: "#f5f5f7",
  },
  ".cm-gutters": {
    backgroundColor: "#050506",
    color: "#52525b",
    borderRight: "1px solid rgba(255, 255, 255, 0.06)",
    paddingLeft: "6px",
  },
  ".cm-activeLine": {
    backgroundColor: "rgba(255, 255, 255, 0.035)",
  },
  ".cm-activeLineGutter": {
    backgroundColor: "transparent",
    color: "#a1a1aa",
  },
  ".cm-selectionBackground, ::selection": {
    backgroundColor: "rgba(10, 132, 255, 0.28)",
  },
  "&.cm-focused": {
    outline: "none",
  },
});

export const LatexEditor: React.FC<LatexEditorProps> = ({ value, onChange }) => {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const viewRef = useRef<EditorView | null>(null);
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;

  useEffect(() => {
    if (!hostRef.current) return;

    const state = EditorState.create({
      doc: value,
      extensions: [
        lineNumbers(),
        history(),
        EditorView.lineWrapping,
        keymap.of([...defaultKeymap, ...historyKeymap]),
        editorTheme,
        EditorView.updateListener.of((update) => {
          if (update.docChanged) {
            onChangeRef.current(update.state.doc.toString());
          }
        }),
      ],
    });

    const view = new EditorView({
      state,
      parent: hostRef.current,
    });
    viewRef.current = view;

    return () => {
      view.destroy();
      viewRef.current = null;
    };
  }, []);

  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    const current = view.state.doc.toString();
    if (current !== value) {
      view.dispatch({
        changes: { from: 0, to: current.length, insert: value },
      });
    }
  }, [value]);

  return <div ref={hostRef} className="codemirror-host" />;
};

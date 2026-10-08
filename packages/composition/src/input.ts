import Template from "./input.pt";
import type { Editor, Template as View } from "./model.ts";

export default function Input<N>(document: Editor<N>): View<N> {
    const view = Template(document) as View<N>;
    document.attachEditor(view.host, view.refs.value, view.refs.caret);
    return view;
};

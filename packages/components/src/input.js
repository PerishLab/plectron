import Template from "./input.pt";

export default function Input(document) {
    const view = Template(document);
    document.attachEditor(view.host, view.refs.value, view.refs.caret);
    return view;
};

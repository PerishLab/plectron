import type { Bookmark, State } from "@plectron/composition/scroll";

export type Node =
    & {
        children: () => Node[];
        focus: () => unknown;
    }
    & Record<"appendChild" | "removeChild", (node: Node) => unknown>
    & Record<"getAttribute", (name: string) => string | null>
    & Record<"setAttribute", (name: string, value: string) => unknown>
    & Record<"setText" | "setValue", (text: string) => unknown>
    & Record<"getValue", () => string>
    & Record<"addEventListener", (name: string, callback: (event: {scroll: State<Node>}) => unknown, options: {scope: {dispose: () => void}}) => unknown>;

export type Document =
    & Record<"createElement", (tag: string) => Node>
    & Record<"createTextNode", (text: string) => Node>
    & Record<"attachEditor", (host: Node, value: Node, caret: Node) => unknown>;

export interface Message {
    id: number;
    author: string;
    avatar: string;
    body: string;
    time: string;
}

export type Position = Bookmark<string> | null;
export type Name = "general" | "engine";

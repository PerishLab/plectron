declare module "*.pt" {
  interface Result {
    host: any;
    refs: Record<string, any>;
    listen?: (handlers: Record<string, (...arguments_: any[]) => unknown>, scope: any) => void;
  }

  const template: (...arguments_: any[]) => Result;
  export default template;
}

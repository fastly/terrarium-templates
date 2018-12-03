import { run_user, Request, Response } from "./http_guest";

function user_entrypoint(req: Request): Response {
    let resp = new Response();
    resp.status = 200;
    resp.body_string = "Hello, world!";
    return resp;
}

export function run(): void {
    run_user(user_entrypoint);
}

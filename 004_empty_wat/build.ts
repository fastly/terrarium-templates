import * as gulp from "gulp";
import { Service, project } from "@wasm/studio-utils";

gulp.task("deploy", async () => {
  const data = await Service.assembleWat(project.getFile("src/main.wat").getData());
  const outWasm = project.newFile("module.wasm", "wasm", true);
  await outWasm.setData(data);
  await Service.deployFiles([project.getFile("module.wasm"), ...project.globFiles("assets/**")], "wasm");
});

gulp.task("default", ["deploy"]);

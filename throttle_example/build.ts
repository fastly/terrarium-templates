import * as gulp from "gulp";
import { Service, project } from "@wasm/studio-utils";

gulp.task("deploy", async () => {
  await Service.deployFiles(project.globFiles("src/**"), "c");
});

gulp.task("default", ["deploy"]);

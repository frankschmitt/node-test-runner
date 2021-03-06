// @flow

const gracefulFs = require('graceful-fs');
const path = require('path');
const Parser = require('./Parser');

function findTests(
  testFilePaths /*: Array<string> */,
  sourceDirs /*: Array<string> */,
  isPackageProject /*: boolean */
) /*: Promise<Array<{ moduleName: string, possiblyTests: Array<string> }>> */ {
  return Promise.all(
    testFilePaths.map((filePath) => {
      const matchingSourceDirs = sourceDirs.filter((dir) =>
        filePath.startsWith(`${dir}${path.sep}`)
      );

      // Tests must be in tests/ or in source-directories – otherwise they won’t
      // compile. Elm won’t be able to find imports.
      switch (matchingSourceDirs.length) {
        case 0:
          return Promise.reject(
            Error(missingSourceDirectoryError(filePath, isPackageProject))
          );

        case 1:
          // Keep going.
          break;

        default:
          // This shouldn’t be possible for package projects.
          return Promise.reject(
            new Error(
              multipleSourceDirectoriesError(filePath, matchingSourceDirs)
            )
          );
      }

      // By finding the module name from the file path we can import it even if
      // the file is full of errors. Elm will then report what’s wrong.
      const moduleNameParts = path
        .relative(matchingSourceDirs[0], filePath)
        .replace(/\.elm$/, '')
        .split(path.sep);
      const moduleName = moduleNameParts.join('.');

      if (!moduleNameParts.every(Parser.isUpperName)) {
        return Promise.reject(
          new Error(
            badModuleNameError(filePath, matchingSourceDirs[0], moduleName)
          )
        );
      }

      return Parser.extractExposedPossiblyTests(
        filePath,
        // We’re reading files asynchronously in a loop here, so it makes sense
        // to use graceful-fs to avoid “too many open files” errors.
        gracefulFs.createReadStream
      ).then((possiblyTests) => ({
        moduleName,
        possiblyTests,
      }));
    })
  );
}

function missingSourceDirectoryError(filePath, isPackageProject) {
  return `
This file:

${filePath}

…matches no source directory! Imports won’t work then.

${
  isPackageProject
    ? 'Move it to tests/ or src/ in your project root.'
    : 'Move it to tests/ in your project root, or make sure it is covered by "source-directories" in your elm.json.'
}
  `.trim();
}

function multipleSourceDirectoriesError(filePath, matchingSourceDirs) {
  return `
This file:

${filePath}

…matches more than one source directory:

${matchingSourceDirs.join('\n')}

Edit "source-directories" in your elm.json and try to make it so no source directory contains another source directory!
  `.trim();
}

function badModuleNameError(filePath, sourceDir, moduleName) {
  return `
This file:

${filePath}

…located in this directory:

${sourceDir}

…is problematic. Trying to construct a module name from the parts after the directory gives:

${moduleName}

…but module names need to look like for example:

Main
Http.Helpers

Make sure that all parts start with an uppercase letter and don’t contain any spaces or anything like that.
  `.trim();
}

module.exports = {
  findTests: findTests,
};

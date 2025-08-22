
/** @type {import('webpack').Configuration} */
let config = {
  target: 'node',
  mode: 'production',
  output: {
    libraryTarget: 'commonjs2',
    devtoolModuleFilenameTemplate: '[resource-path]'
  },
  devtool: 'source-map',
  externals: {
    vscode: 'commonjs vscode',
    mocha: 'commonjs mocha'
  },
  resolve: {
    extensions: ['.js', '.ts', '.tsx'],
    modules: ['${CMAKE_BINARY_DIR}/node_modules', '${CMAKE_SOURCE_DIR}']
  },
  module: {
    rules: [
      {
        test: /\.ts(x?)$/,
        exclude: /node_modules/,
        use: [{ loader: 'ts-loader' }]
      }
    ]
  }
};

export default config;

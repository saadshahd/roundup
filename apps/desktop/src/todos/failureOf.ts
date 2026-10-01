/** The message of a failed call, or `null` when it succeeded. Anything that is not an Error is a bug and escapes. */
export const failureOf = async <Result>(call: () => Promise<Result>): Promise<string | null> => {
  try {
    await call();

    return null;
  } catch (failure) {
    if (!(failure instanceof Error)) throw failure;

    return failure.message;
  }
};

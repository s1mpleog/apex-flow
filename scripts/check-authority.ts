// Run this as a standalone script first to see what's actually on-chain
// scripts/check-authority.ts
import * as anchor from "@coral-xyz/anchor";
import { ApexFlow } from "../target/types/apex_flow";

async function main() {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.ApexFlow as anchor.Program<ApexFlow>;
  const connection = provider.connection;

  console.log("wallet pubkey:    ", provider.wallet.publicKey.toBase58());
  console.log("program id:       ", program.programId.toBase58());

  const programAccountInfo = await connection.getAccountInfo(program.programId);
  if (!programAccountInfo) throw new Error("program account not found");

  // BPFLoaderUpgradeable layout:
  //   [0..4]   discriminator (u32 = 2 means "Program")
  //   [4..36]  programdata_address (Pubkey)
  const programDataPubkey = new anchor.web3.PublicKey(
    programAccountInfo.data.slice(4, 36)
  );
  console.log("programData addr: ", programDataPubkey.toBase58());

  const programDataInfo = await connection.getAccountInfo(programDataPubkey);
  if (!programDataInfo) throw new Error("programData account not found");

  // ProgramData layout:
  //   [0..4]   discriminator (u32 = 3 means "ProgramData")
  //   [4..12]  slot (u64 LE)
  //   [12]     option tag for upgrade_authority (1 byte)
  //   [13..45] upgrade_authority pubkey (if option tag == 1)
  const hasAuthority = programDataInfo.data[12] === 1;
  if (!hasAuthority) {
    console.log("upgrade authority: NONE (immutable program)");
  } else {
    const authority = new anchor.web3.PublicKey(
      programDataInfo.data.slice(13, 45)
    );
    console.log("upgrade authority:", authority.toBase58());
    console.log(
      "wallet == authority?",
      authority.equals(provider.wallet.publicKey)
    );
  }
}

main().catch(console.error);

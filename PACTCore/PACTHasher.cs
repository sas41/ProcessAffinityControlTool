using System;
using System.Collections.Generic;
using System.Text;
using System.Security.Cryptography;

namespace PACTCore
{
    public static class PACTHasher
    {
        private static SHA256 sha256;

        static PACTHasher()
        {
            sha256 = new System.Security.Cryptography.SHA256CryptoServiceProvider();
        }

        // Slightly altered from:
        // https://www.codeproject.com/Articles/34309/Convert-String-to-64bit-Integer
        public static ulong GetUInt64HashCode(string strText)
        {
            ulong hashCode = 0;

            if (!string.IsNullOrEmpty(strText))
            {
                //Unicode Encode Covering all characterset
                byte[] byteContents = Encoding.Unicode.GetBytes(strText);
                byte[] hashText = sha256.ComputeHash(byteContents);
                //32Byte hashText separate
                //hashCodeStart = 0~7  8Byte
                //hashCodeMedium = 8~23  8Byte
                //hashCodeEnd = 24~31  8Byte
                //and Fold
                ulong hashCodeStart  = BitConverter.ToUInt64(hashText, 0);
                ulong hashCodeMedium = BitConverter.ToUInt64(hashText, 8);
                ulong hashCodeEnd    = BitConverter.ToUInt64(hashText, 24);
                hashCode = hashCodeStart ^ hashCodeMedium ^ hashCodeEnd;
            }

            return (hashCode);
        }
    }
}
